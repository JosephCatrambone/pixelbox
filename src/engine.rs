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
use rusqlite::{params, Connection, Error, Result, NO_PARAMS};
use rusqlite::functions::FunctionFlags;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::crawler;
use crate::indexed_image::*;

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

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

#[derive(Clone)]
pub struct Engine {
	pool: Pool<SqliteConnectionManager>,

	// Crawling and indexing:
	crawler_items: Option<crossbeam::channel::Receiver<PathBuf>>, // What images remain to be processed.
	watched_directories_cache: Option<Vec<String>>, // Contains a list of the globs that we monitor.

	// Searching and filtering.
	cached_search_results: Option<Vec<IndexedImage>>,
}

impl Engine {
	pub fn new(filename:&Path) -> Self {
		let conn = Connection::open(filename).expect("Unable to open DB file.");

		// Initialize our image DB and our indices.
		conn.execute(IMAGE_SCHEMA_V1, params![]).unwrap();
		conn.execute(WATCHED_DIRECTORIES_SCHEMA_V1, NO_PARAMS).unwrap();

		conn.execute("CREATE TABLE phashes (id INTEGER PRIMARY KEY, hash BLOB)", params![]).unwrap();
		conn.execute("CREATE TABLE semantic_hash (id INTEGER PRIMARY KEY, hash BLOB)", params![]).unwrap();
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

		Engine {
			pool,
			crawler_items: None,
			watched_directories_cache: None,
			cached_search_results: None,
		}
	}

	pub fn is_indexing_active(&self) -> bool {
		if let Some(rx) = &self.crawler_items {
			if rx.is_empty() {
				false
			} else {
				true
			}
		} else {
			false
		}
	}

	pub fn start_reindexing(&mut self) {
		// Select all our monitored folders and, in parallel, dir walk them to grab new images.
		let all_globs:Vec<String> = self.get_tracked_folders().clone();

		// Image Processing Thread.
		let pool = self.pool.clone();
		let img_rx = crawler::crawl_globs_async(all_globs, 8);
		std::thread::spawn(move || {
			let conn = pool.get().unwrap();
			let mut stmt = conn.prepare("SELECT 1 FROM images WHERE path = ?").unwrap();
			while let Ok(img) = img_rx.recv() {
				println!("Processing {}", &img.path);
				if !stmt.exists(params![&img.path]).unwrap() {
					if let Err(e) = Engine::insert_image(&conn, img) {
						eprintln!("Failed to track image: {}", &e);
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
		conn.execute(
			"INSERT INTO phashes (id, hash) VALUES (?, ?)",
			params![img.id, img.phash.unwrap()]
		)?;
		conn.execute(
			"INSERT INTO semantic_hashes (id, hash) VALUES (?, ?)",
			params![img.id, img.semantic_hash.unwrap()]
		)?;

		Ok(())
	}

	pub fn query_by_image_name(&mut self, text:String) {

	}

	pub fn query_by_image_hash_from_file(&mut self, img:&Path) {
		self.cached_search_results = None;

		let indexed_image = IndexedImage::from_file_path(img).unwrap();

		let conn = self.pool.get().unwrap();
		let mut stmt = conn.prepare(r#"
			SELECT images.id, images.filename, images.path, images.image_width, images.image_height, images.thumbnail, images.thumbnail_width, images.thumbnail_height, hamming_distance(?, image_hashes.hash) AS dist
			FROM semantic_hashes image_hashes
			JOIN images images ON images.id = image_hashes.id
			ORDER BY dist ASC
			LIMIT 100"#).unwrap();
		let img_cursor = stmt.query_map(params![indexed_image.semantic_hash], |row|{
			let img:IndexedImage = IndexedImage {
				id: row.get(0)?,
				filename: row.get(1)?,
				path: row.get(2)?,
				resolution: (row.get(3)?, row.get(4)?),
				thumbnail: row.get(5)?,
				thumbnail_resolution: (row.get(6)?, row.get(7)?),
				created: Instant::now(), //row.get(4)?
				indexed: Instant::now(), //row.get(5)?
				phash: None,
				semantic_hash: None
			};
			Ok(img)
		}).unwrap();

		self.cached_search_results = Some(img_cursor.map(|item|{
			item.unwrap()
		}).collect());
	}

	pub fn get_query_results(&self) -> Option<Vec<IndexedImage>> {
		self.cached_search_results.clone()
	}

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

#[cfg(test)]
mod tests {
	use crate::engine::hamming_distance;

	#[test]
	fn test_hamming_distance() {
		assert_eq!(hamming_distance(&vec![0u8], &vec![0xFFu8]), 1f32);
		assert_eq!(hamming_distance(&vec![0x0Fu8], &vec![0xFFu8]), 0.5f32);
		assert_eq!(hamming_distance(&vec![0x0u8], &vec![0x0u8]), 0.0f32);
		assert_eq!(hamming_distance(&vec![0b10101010u8], &vec![0b01010101u8]), 1f32);
		assert_eq!(hamming_distance(&vec![0b10101010u8, 0b01010101u8], &vec![0b01010101u8, 0b10101010u8]), 1f32);
		assert_eq!(hamming_distance(&vec![0xFFu8, 0x0Fu8], &vec![0x0Fu8, 0x0Fu8]), 0.25f32); // 4 bits are different.
	}
}