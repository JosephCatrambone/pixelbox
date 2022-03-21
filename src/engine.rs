///
/// engine.rs
/// Handles indexing and keeping track of active searches.
/// Closely tied to indexed_image, but indexed_image has a bunch of extra fields, like uncalculated hashes that aren't necessarily stored in the database.
/// Engine manages the spidering, indexing, and keeping track of images.
///

use image::{ImageError, DynamicImage};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rayon::prelude::*;
use rusqlite::{params, Connection, Error, Result, NO_PARAMS, Row, ToSql};
use rusqlite::functions::FunctionFlags;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::crawler;
use crate::indexed_image::*;

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

const PARALLEL_FILE_PROCESSORS: usize = 8;
const DEFAULT_MAX_QUERY_DISTANCE: f64 = 1e6; // f64 implements ToSql in SQLite. f32 doesn't.
const MAX_PENDING_FILEPATHS: usize = 1000;
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
const WATCHED_DIRECTORIES_SCHEMA_V1: &'static str = "CREATE TABLE watched_directories (glob TEXT PRIMARY KEY)";
const HASH_TABLE_SCHEMA_V1: &'static str = "CREATE TABLE ? (image_id INTEGER PRIMARY KEY, hash BLOB)";

fn indexed_image_from_row(row: &Row) -> Result<IndexedImage> {
	Ok(IndexedImage {
		id: row.get(0)?,
		filename: row.get(1)?,
		path: row.get(2)?,
		resolution: (row.get(3)?, row.get(4)?),
		thumbnail: row.get(5)?,
		thumbnail_resolution: (row.get(6)?, row.get(7)?),
		created: Instant::now(), //row.get(8)?
		indexed: Instant::now(), //row.get(9)?
		phash: None,
		visual_hash: None,
		distance_from_query: None,
	})
}

#[derive(Clone)]
pub struct Engine {
	pool: Pool<SqliteConnectionManager>,

	// Crawling and indexing:
	files_pending_processing: Option<crossbeam::channel::Receiver<PathBuf>>, // What images remain to be processed.
	files_pending_storage: Option<crossbeam::channel::Receiver<IndexedImage>>, // What images have been loaded but are not stored.
	files_completed: Option<crossbeam::channel::Receiver<String>>,
	files_failed: Option<crossbeam::channel::Receiver<String>>,
	last_indexed: Vec<String>, // A cache of the last n indexed items.
	watched_directories_cache: Option<Vec<String>>, // Contains a list of the globs that we monitor.

	// Searching and filtering.
	max_distance_from_query: f64,
	cached_search_results: Option<Vec<IndexedImage>>,
}

impl Engine {
	pub fn new(filename:&Path) -> Self {
		let conn = Connection::open(filename).expect("Unable to open DB file.");

		// Initialize our image DB and our indices.
		conn.execute(IMAGE_SCHEMA_V1, params![]).unwrap();
		conn.execute(WATCHED_DIRECTORIES_SCHEMA_V1, NO_PARAMS).unwrap();

		// phashes and semantic hashes should be identical instructure so we can swap them out.
		conn.execute(HASH_TABLE_SCHEMA_V1, params!["phashes",]).unwrap();
		conn.execute(HASH_TABLE_SCHEMA_V1, params!["semantic_hashes",]).unwrap();
		if let Err((_, e)) = conn.close() {
			eprintln!("Failed to close db after table creation: {}", e);
		}

		Engine::open(filename)
	}

	pub fn open(filename:&Path) -> Self {
		let manager = SqliteConnectionManager::file(filename);
		let pool = r2d2::Pool::new(manager).unwrap();

		let conn = pool.get().unwrap();
		make_hamming_distance_db_function(&conn);
		make_byte_distance_db_function(&conn);
		make_cosine_distance_db_function(&conn);

		Engine {
			pool,
			files_pending_processing: None,
			files_pending_storage: None,
			files_completed: None,
			files_failed: None,
			last_indexed: vec![],
			watched_directories_cache: None,
			
			max_distance_from_query: DEFAULT_MAX_QUERY_DISTANCE,
			cached_search_results: None,
		}
	}

	pub fn is_indexing_active(&self) -> bool {
		match (&self.files_pending_processing, &self.files_pending_storage) {
			(Some(f_rx), _) => f_rx.len() > 0,
			(_, Some(img_rx)) => img_rx.len() > 0,
			(None, None) => false
		}
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
		let pool = self.pool.clone();
		let (file_rx, img_rx) = crawler::crawl_globs_async(all_globs, PARALLEL_FILE_PROCESSORS);
		self.files_pending_processing = Some(file_rx.clone());
		self.files_pending_storage = Some(img_rx.clone());
		std::thread::spawn(move || {
			let conn = pool.get().unwrap();
			let mut stmt = conn.prepare("SELECT 1 FROM images WHERE path = ?").unwrap();
			while let Ok(img) = img_rx.recv() {
				if !stmt.exists(params![&img.path]).unwrap() {
					let fname = img.filename.clone();
					if let Err(e) = Engine::insert_image(&conn, img) {
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

	fn insert_image(conn: &PooledConnection<SqliteConnectionManager>, mut img:IndexedImage) -> Result<()> {
		// Update the images table first...
		conn.execute(
			"INSERT INTO images (filename, path, image_width, image_height, thumbnail, thumbnail_width, thumbnail_height) VALUES (?, ?, ?, ?, ?, ?, ?)",
			params![img.filename, img.path, img.resolution.0, img.resolution.1, img.thumbnail, img.thumbnail_resolution.0, img.thumbnail_resolution.1]
		)?;
		img.id = conn.last_insert_rowid();

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

	pub fn query(&mut self, text:&String) {
		// This will parse and process the full query.
		// Magic phrases:
		// filename: matches filename
		// image: or file: does semantic matching
		// tags: matches tags, comma-separated
		// metadata: matches metadata
		// min_width:, max_width:, min_height:, max_height:
		// Absent all that, full-text search on all of these.

		self.cached_search_results = None;

		let conn = self.pool.get().unwrap();
		let mut params:Vec<&dyn ToSql> = vec![];
		let mut fields = "images.id, images.filename, images.path, images.image_width, images.image_height, images.thumbnail, images.thumbnail_width, images.thumbnail_height ".to_string();
		let mut source_table = "images ".to_string();
		let mut order_by = String::new();  // Empty!

		// If there's image: or file: in the query we need to load it and join to the fields.
		if text.contains("file:") || text.contains("image:") {
			fields += "cosine_distance(?, image_hashes.hash) AS dist";
			source_table += "JOIN semantic_hashes image_hashes ON images.id = image_hashes.image_id";
			order_by = "dist DESC".to_string();
			//params.push(indexed_image.visual_hash);
		}

		let mut raw_statement = "SELECT {fields} FROM {source_table} WHERE {where_clause} ORDER BY {order_by}";

		let mut prepared_statement = conn.prepare(raw_statement).unwrap();
		let result_cursor = prepared_statement.query_map(params![], |row| {
			Ok(indexed_image_from_row(row).expect("Unable to decode image in database."))
		}).unwrap();

		self.cached_search_results = result_cursor.map(|item|{
			Some(item.unwrap())
		}).collect();
	}

	pub fn query_by_image_name(&mut self, text:&String) {
		self.cached_search_results = None; // Starting query.

		let conn = self.pool.get().unwrap();
		let mut stmt = conn.prepare(r#"
			SELECT images.id, images.filename, images.path, images.image_width, images.image_height, images.thumbnail, images.thumbnail_width, images.thumbnail_height
			FROM images
			WHERE images.filename LIKE ?1
			LIMIT 100
		"#).unwrap();
		let img_cursor = stmt.query_map(params![text], |row|{
			indexed_image_from_row(row)
		}).unwrap();

		self.cached_search_results = Some(img_cursor.map(|item|{
			item.unwrap()
		}).collect());
	}

	pub fn query_by_image_hash_from_file(&mut self, img:&Path) {
		self.cached_search_results = None;

		let debug_start_load_image = Instant::now();
		let indexed_image = IndexedImage::from_file_path(img).unwrap();
		let debug_end_load_image = Instant::now();

		let debug_start_db_query = Instant::now();
		let conn = self.pool.get().unwrap();
		let mut stmt = conn.prepare(r#"
			SELECT images.id, images.filename, images.path, images.image_width, images.image_height, images.thumbnail, images.thumbnail_width, images.thumbnail_height, cosine_distance(?, image_hashes.hash) AS dist
			FROM semantic_hashes image_hashes
			JOIN images images ON images.id = image_hashes.image_id
			WHERE dist < ?
			ORDER BY dist ASC
			LIMIT 100"#).unwrap();
		let img_cursor = stmt.query_map(params![indexed_image.visual_hash, self.max_distance_from_query], |row|{
			let mut img = indexed_image_from_row(row).expect("Unable to unwrap result from database");
			img.distance_from_query = Some(row.get(8)?);
			Ok(img)
		}).unwrap();

		self.cached_search_results = Some(img_cursor.map(|item|{
			item.unwrap()
		}).collect());
		let debug_end_db_query = Instant::now();
		
		let result_count = self.cached_search_results.as_ref().unwrap().len();

		eprintln!("Time to compute image hash: {:?}.  Time to search DB: {:?}  Results: {:?}", debug_end_load_image-debug_start_load_image, debug_end_db_query-debug_start_db_query, result_count);
	}

	pub fn get_query_results(&self) -> Option<Vec<IndexedImage>> {
		self.cached_search_results.clone()
	}
	
	pub fn clear_query_results(&mut self) { self.cached_search_results = None; }

	pub fn add_tracked_folder(&mut self, folder_glob:String) {
		self.pool.get().unwrap().execute("INSERT INTO watched_directories (glob) VALUES (?1)", params![folder_glob]).unwrap();
		self.watched_directories_cache = None; // Invalidate cache.
		self.get_tracked_folders();
	}

	pub fn remove_tracked_folder(&mut self, folder_glob:String) {
		self.pool.get().unwrap().execute("DELETE FROM watched_directories WHERE glob=?1", params![folder_glob]).unwrap();
		self.watched_directories_cache = None; // Invalidate cache.
		self.get_tracked_folders();
	}

	pub fn get_tracked_folders(&mut self) -> &Vec<String> {
		if self.watched_directories_cache.is_none() {
			let conn = self.pool.get().unwrap();
			let mut stmt = conn.prepare("SELECT glob FROM watched_directories").unwrap();
			let glob_cursor = stmt.query_map(NO_PARAMS, |row|{
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

fn make_cosine_distance_db_function(db: &PooledConnection<SqliteConnectionManager>) -> Result<()> {
	db.create_scalar_function(
		"cosine_distance",
		2,
		FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
		move |ctx| {
			let dist = {
				let lhs = ctx.get_raw(0).as_blob().map_err(|e| Error::UserFunctionError(e.into()))?;
				let rhs = ctx.get_raw(1).as_blob().map_err(|e| Error::UserFunctionError(e.into()))?;
				cosine_distance(&lhs.to_vec(), &rhs.to_vec())
			};
			Ok(dist as f64)
		}
	)
}


fn make_byte_distance_db_function(db: &PooledConnection<SqliteConnectionManager>) -> Result<()> {
	db.create_scalar_function(
		"byte_distance",
		2,
		FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
		move |ctx| {
			let dist = {
				let lhs = ctx.get_raw(0).as_blob().map_err(|e| Error::UserFunctionError(e.into()))?;
				let rhs = ctx.get_raw(1).as_blob().map_err(|e| Error::UserFunctionError(e.into()))?;
				byte_distance(&lhs.to_vec(), &rhs.to_vec())
			};
			Ok(dist as f64)
		}
	)
}

fn make_hamming_distance_db_function(db: &PooledConnection<SqliteConnectionManager>) -> Result<()> {
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
				let lhs = ctx.get_raw(0).as_blob().map_err(|e| Error::UserFunctionError(e.into()))?;
				let rhs = ctx.get_raw(1).as_blob().map_err(|e| Error::UserFunctionError(e.into()))?;
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