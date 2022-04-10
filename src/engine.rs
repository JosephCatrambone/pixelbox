///
/// engine.rs
/// Handles indexing and keeping track of active searches.
/// Closely tied to indexed_image, but indexed_image has a bunch of extra fields, like uncalculated hashes that aren't necessarily stored in the database.
/// Engine manages the spidering, indexing, and keeping track of images.
///

use anyhow::{anyhow, Result};
use crossbeam::channel;
//use rayon::prelude::*;
use parking_lot::FairMutex;
use rusqlite::{params, Connection, Error as SQLError, Result as SQLResult, Row, ToSql, OpenFlags};
use rusqlite::functions::FunctionFlags;
use serde_json::{Result as JSONResult, Value as JSONValue};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::crawler;
use crate::indexed_image::*;

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
type JSONMap = HashMap<String, JSONValue>;

const PARALLEL_FILE_PROCESSORS: usize = 8;
const DEFAULT_MAX_QUERY_DISTANCE: f64 = 1e6; // f64 implements ToSql in SQLite. f32 doesn't.
const MAX_PENDING_FILEPATHS: usize = 1000;

//
// Schemas
// If any of these change we will need to update methods.
//
const IMAGE_SCHEMA_V1: &'static str = "CREATE TABLE images (
	id               INTEGER PRIMARY KEY,
	filename         TEXT NOT NULL,
	path             TEXT NOT NULL,
	image_width      INTEGER,
	image_height     INTEGER,
	thumbnail        BLOB,
	thumbnail_width  INTEGER,
	thumbnail_height INTEGER,
	created          DATETIME,
	indexed          DATETIME
)";
const TAG_SCHEMA_V1: &'static str = "CREATE TABLE tags (
	image_id		INTEGER,
	name			TEXT NOT NULL,
	value			TEXT
)";
const WATCHED_DIRECTORIES_SCHEMA_V1: &'static str = "CREATE TABLE watched_directories (glob TEXT PRIMARY KEY)";
const HASH_TABLE_SCHEMA_V1: &'static str = "CREATE TABLE $tablename$ (image_id INTEGER PRIMARY KEY, hash BLOB)";
// These are all explicitly ordered so they work with indexed_image_from_row.
// Does not include the trailing dist operation or tags.
const SELECT_FIELDS: &'static str = "
	images.id,
	images.filename,
	images.path,
	images.image_width,
	images.image_height,
	images.thumbnail,
	images.thumbnail_width,
	images.thumbnail_height
";
// End Schemas

// We should implement try_from_row for this.
// The last entry is seven, so tags or hashes start at row.get(8).
fn indexed_image_from_row(row: &Row) -> SQLResult<IndexedImage> {
	Ok(IndexedImage {
		id: row.get(0)?,
		filename: row.get(1)?,
		path: row.get(2)?,
		resolution: (row.get(3)?, row.get(4)?),
		thumbnail: row.get(5)?,
		thumbnail_resolution: (row.get(6)?, row.get(7)?),
		created: Instant::now(), //row.get(8)?
		indexed: Instant::now(), //row.get(9)?
		tags: HashMap::new(),
		phash: None,
		visual_hash: None,
		distance_from_query: None,
	})
}

pub struct Engine {
	connection: Arc<FairMutex<Connection>>,

	// Crawling and indexing:
	files_crawled: Option<channel::Receiver<PathBuf>>,
	files_processed: Option<channel::Receiver<IndexedImage>>, // What images have been loaded but are not stored.
	files_completed: Option<channel::Receiver<String>>,
	files_failed: Option<channel::Receiver<String>>,
	last_indexed: Vec<String>, // A cache of the last n indexed items.
	watched_directories_cache: Option<Vec<String>>, // Contains a list of the globs that we monitor.
	cached_index_size: Option<usize>, // Number of indexed images.

	// Searching and filtering.
	max_distance_from_query: f64,
	cached_search_results: Option<Vec<IndexedImage>>,  // For keeping track of the last time a query ran.
	cached_image_search: Option<IndexedImage>, // If the user is searching for a similar image: "similar:abc", this is the path.  We should compare when the abc changes.
}

impl Engine {
	pub fn new(filename:&Path) -> Self {
		let conn = Connection::open(filename).expect("Unable to open DB file.");

		// Initialize our image DB and our indices.
		conn.execute(IMAGE_SCHEMA_V1, params![]).unwrap();
		conn.execute(WATCHED_DIRECTORIES_SCHEMA_V1, []).unwrap();
		conn.execute(TAG_SCHEMA_V1, []).unwrap();

		// phashes and semantic hashes should be identical instructure so we can swap them out.
		// Can't use prepared statements for CREATE TABLE, so we have to substitute $tablename$.
		conn.execute(&HASH_TABLE_SCHEMA_V1.replace("$tablename$", "phashes"), params![]).unwrap();
		conn.execute(&HASH_TABLE_SCHEMA_V1.replace("$tablename$", "semantic_hashes"), params![]).unwrap();
		if let Err((_, e)) = conn.close() {
			eprintln!("Failed to close db after table creation: {}", e);
		}

		Engine::open(filename)
	}

	pub fn open(filename:&Path) -> Self {
		let mut conn = Connection::open(filename).expect("Unable to open filename.");

		make_hamming_distance_db_function(&mut conn);
		make_byte_distance_db_function(&mut conn);
		make_cosine_distance_db_function(&mut conn);

		Engine {
			connection: Arc::new(FairMutex::new(conn)),
			files_crawled: None,
			files_processed: None,
			files_completed: None,
			files_failed: None,
			last_indexed: vec![],
			watched_directories_cache: None,
			cached_index_size: None,
			
			max_distance_from_query: DEFAULT_MAX_QUERY_DISTANCE,
			cached_search_results: None,
			cached_image_search: None,
		}
	}

	pub fn is_indexing_active(&self) -> bool {
		match (&self.files_crawled, &self.files_processed) {
			(Some(f_rx), _) => f_rx.len() > 0,
			(_, Some(img_rx)) => img_rx.len() > 0,
			(None, None) => false
		}
	}

	pub fn get_indexing_progress(&self) -> f32 {
		let num_unread = if let Some(fname_rx) = &self.files_crawled { fname_rx.len() } else { 0 };
		let num_unprocessed = if let Some(img_rx) = &self.files_processed { img_rx.len() } else { 0 };
		let num_completed = if let Some(cmp) = &self.files_completed { cmp.len() } else { 0 };
		let num_indexed = self.try_get_num_indexed_images().unwrap_or(0);
		(num_indexed as f32) / 1.0f32.max((num_indexed + num_unread + num_unprocessed + num_completed) as f32)
	}

	pub fn try_get_num_indexed_images(&self) -> Option<usize> {
		self.cached_index_size
	}

	/// Perform a count of the number of indexed images and cache the value.
	pub fn get_num_indexed_images(&mut self) -> usize {
		let conn = self.connection.lock();
		let mut stmt = conn.prepare("SELECT COUNT(*) FROM images").unwrap();
		let num_rows_iter = stmt.query_map([], |row|{
			Ok(row.get(0)?)
		}).expect("Unable to count rows in image database");
		for nr in num_rows_iter {
			self.cached_index_size = Some(nr.unwrap());
		}
		self.cached_index_size.unwrap()
	}

	pub fn get_last_indexed(&mut self) -> &Vec<String> {
		if let Some(rx) = &self.files_completed {
			while let Ok(msg) = rx.recv_timeout(Duration::from_nanos(1)) {
				self.last_indexed.push(msg);
			}
		}

		// Cap last indexed to 10.
		while self.last_indexed.len() > 10 {
			self.last_indexed.remove(0);
		}

		&self.last_indexed
	}

	pub fn start_reindexing(&mut self) {
		// How this works:
		// We select all our tracked folders from the database, then open a multi-stage pipeline:
		// The crawl_globs_async begins to parallel crawl the filenames.
		// As filenames are read, they're sent to the files_pending_processing queue.
		// As files are read and converted into images they're sent to the files_pending_storage queue.
		// Another thread (here) checks if images are already in the database and, if not, inserts them.
		// Successes/failures to insert are reported to failure_tx/success_tx.
		
		// Select all our monitored folders and, in parallel, dir walk them to grab new images.
		let all_globs:Vec<String> = self.get_tracked_folders().clone();

		let (success_tx, success_rx) = crossbeam::channel::unbounded();
		self.files_completed = Some(success_rx);
		let (failure_tx, failure_rx) = crossbeam::channel::unbounded();
		self.files_failed = Some(failure_rx);

		// Image Processing Thread.
		// file_rx / files_pending_processing
		// img_rx / files_pending_storage
		let (file_rx, img_rx) = crawler::crawl_globs_async(all_globs, PARALLEL_FILE_PROCESSORS);
		self.files_crawled = Some(file_rx.clone());
		self.files_processed = Some(img_rx.clone());
		let w_conn = self.connection.clone();
		std::thread::spawn(move || {
			// To hold the lock as briefly as possible, we grab reads and writes very briefly.
			// There is some overhead associated with getting the writes, so we might have to invert this pattern later.
			while let Ok(img) = img_rx.recv() {
				// Hold a short read lock and check if the image is already in our index.
				let exists = {
					let conn = w_conn.lock();
					let mut stmt = conn.prepare("SELECT 1 FROM images WHERE path = ?").unwrap();
					stmt.exists(params![&img.path]).unwrap()
				};
				// Image is not in our index.  Add it!
				if !exists {
					let fname = img.filename.clone();
					// Quickly lock and unlock.
					let insert_result = {
						let mut rw_conn = w_conn.lock();
						Engine::insert_image(&mut rw_conn, img)
					};
					if let Err(e) = insert_result {
						eprintln!("Failed to track image: {}", &e);
						failure_tx.send(format!("{}: {}", fname, e));
					} else {
						success_tx.send(fname);
					}
				};
			}
			//conn.flush_prepared_statement_cache();
		});
	}

	//fn get_reindexing_status(&self) -> bool {}

	fn insert_image(conn: &mut Connection, mut img:IndexedImage) -> Result<()> {
		// Update the images table first...
		conn.execute(
			"INSERT INTO images (filename, path, image_width, image_height, thumbnail, thumbnail_width, thumbnail_height) VALUES (?, ?, ?, ?, ?, ?, ?)",
			params![img.filename, img.path, img.resolution.0, img.resolution.1, img.thumbnail, img.thumbnail_resolution.0, img.thumbnail_resolution.1]
		)?;
		img.id = conn.last_insert_rowid();

		// Insert the tags.
		img.tags.iter().for_each(|(tag_name, tag_value)| {
			conn.execute(
				"INSERT INTO tags (image_id, name, value) VALUES (?, ?, ?)",
				params![&img.id, tag_name, tag_value]
			).expect(&format!("Failed to insert tag into database for image ID {}", &img.id));
		});

		// Add the hashes.
		if let Some(hash) = img.phash {
			conn.execute(
				"INSERT INTO phashes (image_id, hash) VALUES (?, ?)",
				params![img.id, hash]
			)?;
		}
		if let Some(hash) = img.visual_hash {
			conn.execute(
				"INSERT INTO semantic_hashes (image_id, hash) VALUES (?, ?)",
				params![img.id, hash]
			)?;
		}

		Ok(())
	}

	pub fn query(&mut self, user_input:&String) -> Result<()> {
		// This will parse and process the full query.
		// Magic phrases:
		// filename: matches filename
		// image: or file: does semantic matching
		// tags: matches tags, comma-separated
		// metadata: matches metadata
		// min_width:, max_width:, min_height:, max_height:
		// Absent all that, full-text search on all of these.

		if user_input.is_empty() {
			return Ok(()); // Bail early!
			// TODO: Should we clear results?
		}

		let mut parameters = params![];
		let parsed_query = tokenize_query(user_input)?;
		let where_clause = build_where_clause_from_parsed_query(&parsed_query, &mut self.cached_image_search);

		self.cached_search_results = None;

		let included_distance_hash = match &self.cached_image_search {
			Some(img) => {
				if let Some(hash) = &img.visual_hash {
					parameters = params![hash];
					"cosine_distance(?, semantic_hashes.hash)"
				} else {
					"0.0"
				}
			},
			None => "0.0"
		};

		let mut statement = format!("
			WITH grouped_tags AS (
				SELECT tags.image_id, JSON(JSON_GROUP_ARRAY(JSON_OBJECT(
					tags.name, tags.value
				))) as tags
				FROM tags
				GROUP BY tags.image_id
			)
			SELECT
				{},
				semantic_hashes.hash,
				grouped_tags.tags,
				{} AS dist
			FROM images
			INNER JOIN semantic_hashes ON images.id = semantic_hashes.image_id
			LEFT JOIN grouped_tags ON images.id = grouped_tags.image_id
			LEFT JOIN tags ON images.id = tags.image_id
			WHERE {}
			GROUP BY images.id
			ORDER BY dist ASC
			LIMIT 100;
		", SELECT_FIELDS, included_distance_hash, where_clause);

		// Grab a read lock.
		self.cached_search_results = {
			let conn = self.connection.lock();

			// Try and perform the user's query (or some version of our assembled query).
			let mut prepared_statement = conn.prepare(&statement)?;

			// Parse and process results.
			let result_cursor = prepared_statement.query_map(params![], |row| {
				let mut img = indexed_image_from_row(row).expect("Unable to decode image in database.");
				img.visual_hash = row.get(8).ok();
				img.tags = HashMap::new();
				let maybe_tag_data: SQLResult<JSONValue> = row.get(9);
				if let Ok(tag_data) = maybe_tag_data {
					println!("TAG DATA: {}", &tag_data.to_string());
					// TODO: The returned JSON is a bit messy.  It's a Vec of single-key-single-value items.
					// Example: "[{\"ImageWidth\":\"2592\"},{\"BitsPerSample\":\"8, 8, 8\"},{\"YCbCrPositioning\":\"centered\"},{\"DateTimeOriginal\":\"2012-10-10 10:49:15\"},{\"DateTimeDigitized\":\"2002-12-08 12:00:00\"},{\"JPEGInterchangeFormatLength\":\"8663\"},...
					// When we clean up the query we should clean up this method.
					if let Some(map_obj) = tag_data.as_object() {
						for (k, v) in map_obj.iter() {
							img.tags.insert(k.to_string(), v.to_string());
						}
					}
				}
				img.distance_from_query = row.get(10).ok();
				Ok(img)
			})?;

			result_cursor.map(|item| {
				Some(item.unwrap())
			}).collect()
		};
		
		println!("{} results", &self.cached_search_results.as_ref().unwrap().len());

		Ok(())
	}

	pub fn query_by_image_hash_from_file(&mut self, img:&Path) {
		self.cached_search_results = None;

		let debug_start_load_image = Instant::now();
		let indexed_image = IndexedImage::from_file_path(img).unwrap();
		let debug_end_load_image = Instant::now();
		eprintln!("Time to compute image hash: {:?}", debug_end_load_image-debug_start_load_image);

		self.query_by_image_hash_from_image(&indexed_image);
	}

	pub fn query_by_image_hash_from_image(&mut self, indexed_image:&IndexedImage) {
		if indexed_image.visual_hash.is_none() {
			// TODO: Error-handling here.
			eprintln!("TODO: IndexedImage is somehow missing a hash!");
			return;
		}

		self.cached_search_results = None;

		let debug_start_db_query = Instant::now();
		let conn = self.connection.lock();
		let mut stmt = conn.prepare(&format!(r#"
			SELECT {}, semantic_hashes.hash, cosine_distance(?, semantic_hashes.hash) AS dist
			FROM semantic_hashes
			INNER JOIN images images ON images.id = semantic_hashes.image_id
			WHERE dist < ?
			ORDER BY dist ASC
			LIMIT 100"#, SELECT_FIELDS
		)).expect("The query for query_by_image_hash_from_image is wrong! The developer messed up!");
		let img_cursor = stmt.query_map(params![indexed_image.visual_hash, self.max_distance_from_query], |row|{
			let mut img = indexed_image_from_row(row).expect("Unable to unwrap result from database");
			img.visual_hash = Some(row.get(8)?);
			img.distance_from_query = Some(row.get(9)?);
			Ok(img)
		}).unwrap();

		self.cached_search_results = Some(img_cursor.flat_map(|item| item).collect());
		let debug_end_db_query = Instant::now();
		
		let result_count = self.cached_search_results.as_ref().unwrap().len();

		eprintln!("Time to search DB: {:?}  Results: {:?}", debug_end_db_query-debug_start_db_query, result_count);
	}

	pub fn get_query_results(&self) -> Option<Vec<IndexedImage>> {
		self.cached_search_results.clone()
	}
	
	pub fn clear_query_results(&mut self) { self.cached_search_results = None; }

	pub fn add_tracked_folder(&mut self, folder_glob:String) {
		{
			self.connection.lock().execute("INSERT INTO watched_directories (glob) VALUES (?1)", params![folder_glob]).unwrap();
		}
		self.watched_directories_cache = None; // Invalidate cache.
		self.get_tracked_folders();
	}

	pub fn remove_tracked_folder(&mut self, folder_glob:String) {
		{
			self.connection.lock().execute("DELETE FROM watched_directories WHERE glob=?1", params![folder_glob]).unwrap();
		}
		self.watched_directories_cache = None; // Invalidate cache.
		self.get_tracked_folders();
	}

	pub fn get_tracked_folders(&mut self) -> &Vec<String> {
		if self.watched_directories_cache.is_none() {
			let conn = self.connection.lock();
			let mut stmt = conn.prepare("SELECT glob FROM watched_directories").unwrap();
			let glob_cursor = stmt.query_map([], |row|{
				let dir:String = row.get(0)?;
				Ok(dir)
			}).unwrap();

			let all_globs:Vec<String> = glob_cursor.map(|item|{
				item.unwrap()
			}).collect();

			self.watched_directories_cache = Some(all_globs);
		}

		if let Some(watched) = &self.watched_directories_cache {
			watched
		} else {
			unreachable!()
		}
	}
}

// Query utility functions:
fn tokenize_query(query: &String) -> Result<Vec<String>> {
	let mut spans = vec![];
	let mut next_character_escaped = false;
	let mut quote_active = false;
	let mut active_string = String::new(); // We accumulate into this, stopping at a space if not quoted or stopping at an end-quote if quoted.
	for character in query.chars() {
		if next_character_escaped {
			active_string.push(character);
			next_character_escaped = false;
		} else {
			match character {
				'"' => {
					// This double is NOT quoted, so we are either starting or finishing a quote.
					if !quote_active { // We are starting.
						quote_active = true;
					} else { // We are finishing a quote.
						quote_active = false;
						spans.push(active_string);
						active_string = String::new();
					}
				},
				'\\' => {
					// A backtick!
					next_character_escaped = true;
				},
				' ' => {
					// If we are in a quote, continue.  Otherwise break the word.
					if quote_active {
						active_string.push(' ');
					} else {
						// We are at a breakpoint, but if the active word is empty there's no sense in pushing it.
						if !active_string.is_empty() {
							spans.push(active_string);
							active_string = String::new();
						}
					}
				},
				_ => active_string.push(character)
			}
		}
	};

	if quote_active {
		return Err(anyhow!("String tokenization failed: trailing open-quote.".to_string()));
	} else if next_character_escaped {
		return Err(anyhow!("String tokenization failed: trailing escape character.".to_string()));
	}

	// Push the last trailing active string into the spans.
	if !active_string.is_empty() {
		spans.push(active_string);
	}

	Ok(spans)
}

fn build_where_clause_from_parsed_query(tokens: &Vec<String>, mut cached_similar_image: &mut Option<IndexedImage>) -> String {
	// If there's a magic prefix like "similar", "filename", or a tag, add that to a 'where'.
	// Otherwise, search all of the tags and exif data.

	let mut and_where_clauses = vec![];
	for token in tokens {
		if let Some((magic_prefix, remaining)) = token.split_once(':') {
			let magic_prefix = magic_prefix.to_string().to_lowercase();
			// SPECIAL CASE FOR VISUAL SIMILARITY!
			// I hate that this is separate and would like to clean up this method.
			// It's kinda' a different modality of searching.
			if magic_prefix.eq("similar") {
				// If we already hashed this image and it is unchanged, don't recalculate.
				let mut needs_recalculation = false;

				// If there's no cached image, obviously we need to recalculate.
				if cached_similar_image.is_none() {
					needs_recalculation = true;
				}

				// If the cached image is different to the last one, we need to recalc.
				if let Some(img) = cached_similar_image {
					// TODO: For case-sensitive operating systems this might need to change.
					if !img.path.eq_ignore_ascii_case(remaining) {
						needs_recalculation = true;
					}
				}

				if needs_recalculation {
					let debug_start_load_image = Instant::now();
					let indexed_image = IndexedImage::from_file_path(Path::new(remaining));
					let debug_end_load_image = Instant::now();
					eprintln!("Time to compute image hash: {:?}", debug_end_load_image - debug_start_load_image);
					*cached_similar_image = indexed_image.ok();
				}
			}

			if magic_prefix.eq("exif") {
				// Split the remaining into tag and target.
				// If there's no ':' then search both.
				if let Some((tag, target)) = remaining.split_once(":") {
					and_where_clauses.push(format!("(tags.name LIKE '%{}%' AND tags.value LIKE '%{}%')", tag, target));
				} else {
					and_where_clauses.push(format!("(tags.name LIKE '%{}%' OR tags.value LIKE '%{}%')", &remaining, &remaining));
				}
			}

			if magic_prefix.eq("filename") {
				and_where_clauses.push(format!("images.filename LIKE '%{}%'", &remaining));
			}
		} else {
			// Search for this value in EVERY field.
			// TODO: We should use '?', though it's not a security vulnerability because it's a strictly local DB.
			and_where_clauses.push(format!(" (tags.value LIKE '%{}%' OR images.filename LIKE '%{}%' OR images.path LIKE '%{}%') ", token, token, token));
		}
	}

	and_where_clauses.join(" AND ")
}

//
// Distance Functions
// Distance functions should return near zero for almost identical items and a large value for different ones.
// All of these methods should take the encoded hash as a blob of u8's and return a single f32.
//
pub fn cosine_distance(hash_a:&Vec<u8>, hash_b:&Vec<u8>) -> f32 {
	// Cosine Similarity -> 1.0 is most similar, -1.0 is most different.
	// We want 0.0 is most similar.
	let u8_to_float = |u8s: &[u8]| {
		u8s.iter().map(|v| { ((*v as f32 / 255.0) * 2.0) - 1.0 }).collect::<Vec<f32>>()
	};
	let hash_a = u8_to_float(hash_a);
	let hash_b = u8_to_float(hash_b);
	let mag_op = |initial, x| { initial + x*x };
	let magnitude = hash_a.iter().fold(0f32, mag_op).sqrt() * hash_b.iter().fold(0f32, mag_op).sqrt();
	if magnitude < 1e-6 {
		return 0.0;
	}
	let dot = hash_a.iter().zip(&hash_b).fold(0f32, |initial, (&a, &b)| { initial + (a*b) });
	let cosine_similarity = dot / magnitude;
	(1.0 / cosine_similarity.max(1e-6)) - 1.0
}

pub fn byte_distance(hash_a:&Vec<u8>, hash_b:&Vec<u8>) -> f32 {
	hash_a.iter().zip(hash_b).fold(0f32, |init, (&a, &b)|{init + (a as f32 - b as f32).abs()}) / (255f32 * hash_a.len() as f32)
}

pub fn hamming_distance(hash_a:&Vec<u8>, hash_b:&Vec<u8>) -> f32 {
	hash_a.iter().zip(hash_b).map(|(&a, &b)|{
		let mut diff = a ^ b;
		let mut bits_set = 0;
		while diff != 0 {
			bits_set += diff & 1;
			diff >>= 1;
		}
		bits_set
	}).sum::<u8>() as f32 / (8f32 * hash_a.len() as f32)
}

// Add all the wrappers to the SQLite functions so we can use them in the database.

fn make_cosine_distance_db_function(db: &mut Connection) -> SQLResult<()> {
	db.create_scalar_function(
		"cosine_distance",
		2,
		FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
		move |ctx| {
			let dist = {
				let lhs = ctx.get_raw(0).as_blob().map_err(|e| SQLError::UserFunctionError(e.into()))?;
				let rhs = ctx.get_raw(1).as_blob().map_err(|e| SQLError::UserFunctionError(e.into()))?;
				cosine_distance(&lhs.to_vec(), &rhs.to_vec())
			};
			Ok(dist as f64)
		}
	)
}

fn make_byte_distance_db_function(db: &mut Connection) -> SQLResult<()> {
	db.create_scalar_function(
		"byte_distance",
		2,
		FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
		move |ctx| {
			let dist = {
				let lhs = ctx.get_raw(0).as_blob().map_err(|e| SQLError::UserFunctionError(e.into()))?;
				let rhs = ctx.get_raw(1).as_blob().map_err(|e| SQLError::UserFunctionError(e.into()))?;
				byte_distance(&lhs.to_vec(), &rhs.to_vec())
			};
			Ok(dist as f64)
		}
	)
}

fn make_hamming_distance_db_function(db: &mut Connection) -> SQLResult<()> {
	//fn make_hamming_distance_db_function(db: &Connection) -> Result<()> {
	db.create_scalar_function(
		"hamming_distance",
		2,
		FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
		move |ctx| {
			assert_eq!(ctx.len(), 2, "Called with incorrect number of arguments");
			/*
			let base_blob: Arc<Vec<u8>> = ctx
				.get_or_create_aux(0, |vr| -> Result<_, BoxError> {
					Ok(Vec::new(vr.as_blob()?)?)
				})?;
			*/
			// This repeatedly grabs and regenerates the LHS.  We should change it.
			let distance = {
				let lhs = ctx.get_raw(0).as_blob().map_err(|e| SQLError::UserFunctionError(e.into()))?;
				let rhs = ctx.get_raw(1).as_blob().map_err(|e| SQLError::UserFunctionError(e.into()))?;
				hamming_distance(&lhs.to_vec(), &rhs.to_vec())
			};
			Ok(distance as f64)
		},
	)
}

// End Distance Functions

#[cfg(test)]
mod tests {
	use crate::engine::hamming_distance;
	use crate::engine::cosine_distance;
	use crate::engine::tokenize_query;

	#[test]
	fn test_tokenize_query() {
		let mut tokens;

		tokens = tokenize_query(&"abc".to_string()).unwrap();
		assert_eq!(tokens, vec!["abc".to_string()]);

		tokens = tokenize_query(&"abc def".to_string()).unwrap();
		assert_eq!(tokens, vec!["abc".to_string(), "def".to_string()]);

		tokens = tokenize_query(&r#"abc "def ghi""#.to_string()).unwrap();
		assert_eq!(tokens, vec!["abc".to_string(), "def ghi".to_string()]);

		tokens = tokenize_query(&r#"abc \"def ghi\""#.to_string()).unwrap();
		assert_eq!(tokens, vec!["abc".to_string(), "\"def".to_string(), "ghi\"".to_string()]);

		tokens = tokenize_query(&r#""the human torch was denied a bank loan" "the \"human torch\"""#.to_string()).unwrap();
		assert_eq!(tokens, vec!["the human torch was denied a bank loan".to_string(), "the \"human torch\"".to_string()]);
	}

	#[test]
	fn test_hamming_distance() {
		assert_eq!(hamming_distance(&vec![0u8], &vec![0xFFu8]), 1f32);
		assert_eq!(hamming_distance(&vec![0x0Fu8], &vec![0xFFu8]), 0.5f32);
		assert_eq!(hamming_distance(&vec![0x0u8], &vec![0x0u8]), 0.0f32);
		assert_eq!(hamming_distance(&vec![0b10101010u8], &vec![0b01010101u8]), 1f32);
		assert_eq!(hamming_distance(&vec![0b10101010u8, 0b01010101u8], &vec![0b01010101u8, 0b10101010u8]), 1f32);
		assert_eq!(hamming_distance(&vec![0xFFu8, 0x0Fu8], &vec![0x0Fu8, 0x0Fu8]), 0.25f32); // 4 bits are different.
	}
	
	#[test]
	fn test_cosine_distance() {
		assert!(cosine_distance(&vec![255u8, 0], &vec![255u8, 0]) < 1e-6f32); // <1,-1> . <1,-1> -> 2 /
		assert!(cosine_distance(&vec![0, 255], &vec![0, 255]) < 1e-6f32);
		assert!(cosine_distance(&vec![255, 0], &vec![0, 255]) > 2.0f32);
	}
}