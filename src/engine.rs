///
/// engine.rs
/// Handles indexing and keeping track of active searches.
/// Closely tied to indexed_image, but indexed_image has a bunch of extra fields, like uncalculated hashes that aren't necessarily stored in the database.
/// Engine manages the spidering, indexing, and keeping track of images.
///

use anyhow::{anyhow, Result};
//use rayon::prelude::*;
use rusqlite::{params, Connection, Error as SQLError, Result as SQLResult, Row, ToSql, OpenFlags};
use rusqlite::functions::FunctionFlags;
use serde_json::{Value as JSONValue};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crossbeam::channel::{Receiver, TryRecvError};
use crate::crawler::Crawler;
use crate::indexed_image::*;

const PARALLEL_FILE_PROCESSORS: usize = 4;
const DEFAULT_MAX_QUERY_DISTANCE: f64 = 1e3; // f64 implements ToSql in SQLite. f32 doesn't.
const DEFAULT_MAX_SEARCH_RESULTS: u64 = 100;
const RECENT_IMAGES_TO_SHOW: usize = 10; // How many of the recently indexed images should we display?

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
	created          DATETIME,
	indexed          DATETIME,
	UNIQUE(path)
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
	images.thumbnail
";
// End Schemas

// We should implement try_from_row for this.
// The last entry is seven, so tags or hashes start at row.get(6).
fn indexed_image_from_row(row: &Row) -> SQLResult<IndexedImage> {
	Ok(IndexedImage {
		id: row.get(0)?,
		filename: row.get(1)?,
		path: row.get(2)?,
		resolution: (row.get(3)?, row.get(4)?),
		thumbnail: row.get(5)?,
		created: Instant::now(), //row.get(6)?
		indexed: Instant::now(), //row.get(7)?
		tags: HashMap::new(),
		phash: None,
		visual_hash: None,
		distance_from_query: None,
	})
}

pub struct Engine {
	connection: Arc<Mutex<Connection>>,
	crawler_channel: Option<Receiver<IndexedImage>>,

	// Crawling and indexing:
	watched_directories_cache: Option<Vec<String>>, // Contains a list of the globs that we monitor.
	cached_index_size: Option<usize>, // Number of indexed images.
	recently_indexed: Vec<String>,

	// Searching and filtering.
	pub max_search_results: u64,
	pub max_distance_from_query: f64,
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
			connection: Arc::new(Mutex::new(conn)),
			crawler_channel: None,
			watched_directories_cache: None,
			cached_index_size: None,
			recently_indexed: vec![],

			max_search_results: DEFAULT_MAX_SEARCH_RESULTS,
			max_distance_from_query: DEFAULT_MAX_QUERY_DISTANCE,
			cached_search_results: None,
			cached_image_search: None,
		}
	}

	pub fn try_get_num_indexed_images(&self) -> Option<usize> {
		self.cached_index_size
	}

	/// Perform a count of the number of indexed images and cache the value.
	pub fn get_num_indexed_images(&mut self) -> usize {
		let conn = self.connection.lock().expect("Failed to lock DB connection while getting index data.");
		let mut stmt = conn.prepare("SELECT COUNT(*) FROM images").unwrap();
		let num_rows_iter = stmt.query_map([], |row|{
			Ok(row.get::<_, i64>(0)? as usize)
		}).expect("Unable to count rows in image database");
		for nr in num_rows_iter {
			self.cached_index_size = Some(nr.unwrap());
		}
		self.cached_index_size.unwrap()
	}

	pub fn is_indexing_active(&self) -> bool {
		self.crawler_channel.is_some()
	}

	pub fn start_indexing(&mut self) {
		let tracked_folders = self.get_tracked_folders().clone();
		let mut crawler = Crawler::new();
		let channel = crawler.start_indexing(tracked_folders, PARALLEL_FILE_PROCESSORS);
		self.crawler_channel = Some(channel);

		{
			let db_ref = self.connection.clone();
			let channel = self.crawler_channel.as_ref().unwrap().clone();
			std::thread::spawn(move || {
				let indexing_start_time = Instant::now();
				'end: loop {
					match channel.try_recv() {
						Ok(img) => {
							let mut db = db_ref.lock().unwrap();
							Engine::insert_image_from_connection(&mut db, img).expect("Failed to insert image.");
						},
						Err(TryRecvError::Empty) => {
							std::thread::yield_now(); // Chill for a while.
						},
						Err(TryRecvError::Disconnected) => {
							break 'end;
						}
					}
				}
				let indexing_completion_time = Instant::now();
				println!("Indexing took {:?} seconds.", (indexing_completion_time-indexing_start_time).as_secs())
			});
		}
	}

	pub fn stop_indexing(&mut self) {
		let c = self.crawler_channel.take();
		if let Some(c) = c {
			drop(c);
		}
	}

	pub fn get_last_added(&self) -> &Vec<String> {
		&self.recently_indexed
	}

	pub fn insert_image_from_path(&mut self, path: &Path) -> Result<()> {
		let img = IndexedImage::from_file_path(path)?;
		self.insert_image_from_memory(img)?;
		Ok(())
	}

	pub fn insert_image_from_memory(&mut self, img:IndexedImage) -> Result<()> {
		let mut conn = self.connection.lock().expect("Failed to lock DB connection while getting index data.");
		Engine::insert_image_from_connection(&mut conn, img)
	}

	fn insert_image_from_connection(conn: &mut Connection, mut img:IndexedImage) -> Result<()> {
		// Update the images table first...
		conn.execute(
			"INSERT OR IGNORE INTO images (filename, path, image_width, image_height, thumbnail) VALUES (?, ?, ?, ?, ?)",
			params![img.filename, img.path, img.resolution.0, img.resolution.1, img.thumbnail,]
		)?;
		img.id = conn.last_insert_rowid();

		// Insert the tags.
		img.tags.iter().for_each(|(tag_name, tag_value)| {
			conn.execute(
				"INSERT OR IGNORE INTO tags (image_id, name, value) VALUES (?, ?, ?)",
				params![&img.id, tag_name, tag_value]
			).expect(&format!("Failed to insert tag into database for image ID {}", &img.id));
		});

		// Add the hashes.
		if let Some(hash) = img.phash {
			conn.execute(
				"INSERT OR IGNORE INTO phashes (image_id, hash) VALUES (?, ?)",
				params![img.id, hash]
			)?;
		}
		if let Some(hash) = img.visual_hash {
			conn.execute(
				"INSERT OR IGNORE INTO semantic_hashes (image_id, hash) VALUES (?, ?)",
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

		let statement = format!("
			WITH grouped_tags AS (
				SELECT tags.image_id, JSON(JSON_GROUP_OBJECT(
					tags.name, tags.value
				)) as tags
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
			let conn = self.connection.lock().expect("Failed to lock DB while querying.");

			// Try and perform the user's query (or some version of our assembled query).
			let mut prepared_statement = conn.prepare(&statement)?;

			// Parse and process results.
			let result_cursor = prepared_statement.query_map(params![], |row| {
				let mut img = indexed_image_from_row(row).expect("Unable to decode image in database.");
				img.visual_hash = row.get(6).ok();
				img.tags = HashMap::new();
				let maybe_tag_data: SQLResult<JSONValue> = row.get(7);
				if let Ok(tag_data) = maybe_tag_data {
					if let Some(map_obj) = tag_data.as_object() {
						for (k, v) in map_obj.iter() {
							img.tags.insert(k.to_string(), v.to_string());
						}
					}
				}
				img.distance_from_query = row.get(8).ok();
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
		let conn = self.connection.lock().expect("Failed to lock DB while querying.");
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
			img.visual_hash = Some(row.get(6)?);
			img.distance_from_query = Some(row.get(7)?);
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
			self.connection.lock().unwrap().execute("INSERT INTO watched_directories (glob) VALUES (?1)", params![folder_glob]).unwrap();
		}
		self.watched_directories_cache = None; // Invalidate cache.
		self.get_tracked_folders();
	}

	pub fn remove_tracked_folder(&mut self, folder_glob:String) {
		{
			self.connection.lock().unwrap().execute("DELETE FROM watched_directories WHERE glob=?1", params![folder_glob]).unwrap();
		}
		self.watched_directories_cache = None; // Invalidate cache.
		self.get_tracked_folders();
		// TODO: Remove images which exist inside the indexed folder.
	}

	pub fn get_tracked_folders(&mut self) -> &Vec<String> {
		if self.watched_directories_cache.is_none() {
			let conn = self.connection.lock().expect("Failed to lock DB while fetching tracked folders.");
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
				
				//and_where_clauses.push(format!("()"));
				// TODO: Do we need to append this actually?
			}

			if magic_prefix.eq("exif") || magic_prefix.eq("tag") {
				// Split the remaining into tag and target.
				// If there's no ':' then search both.
				if let Some((tag, target)) = remaining.split_once(":") {
					and_where_clauses.push(format!("(tags.name LIKE '%{}%' AND tags.value LIKE '%{}%')", tag, target));
				} else {
					and_where_clauses.push(format!("(tags.name LIKE '%{}%' OR tags.value LIKE '%{}%')", &remaining, &remaining));
				}
			}

			if magic_prefix.eq("all") {
				// Search for this value in EVERY field.
				// TODO: We should use '?', though it's not a security vulnerability because it's a strictly local DB.
				and_where_clauses.push(format!(" (tags.value LIKE '%{}%' OR images.filename LIKE '%{}%' OR images.path LIKE '%{}%') ", &remaining, &remaining, &remaining));
			}

			// We default to filename but want to handle the case where the person explicitly searches for it.
			if magic_prefix.eq("filename") {
				and_where_clauses.push(format!("images.filename LIKE '%{}%'", &token));
			}
		} else {
			and_where_clauses.push(format!("images.filename LIKE '%{}%'", &token));
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