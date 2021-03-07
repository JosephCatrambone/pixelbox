///
/// engine.rs
/// Handles indexing and keeping track of active searches.
/// Closely tied to indexed_image, but indexed_image has a bunch of extra fields, like uncalculated hashes that aren't necessarily stored in the database.
/// Engine manages the spidering, indexing, and keeping track of images.
///

use glob::glob;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rayon::prelude::*;
use rusqlite::{params, Connection, Error, Result, NO_PARAMS};
use rusqlite::functions::FunctionFlags;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::indexed_image::*;

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

const MAX_PENDING_FILEPAHTS: usize = 1000;
const IMAGE_SCHEMA_V1: &'static str = "CREATE TABLE images (
	id             INTEGER PRIMARY KEY,
	filename       TEXT NOT NULL,
	path           TEXT NOT NULL,
	thumbnail      BLOB,
	created        DATETIME,
	indexed        DATETIME
)";

#[derive(Clone)]
pub struct Engine {
	pool: Pool<SqliteConnectionManager>,
	crawler_items: Option<crossbeam::channel::Receiver<PathBuf>>, // What images remain to be processed.
}

impl Engine {
	pub fn new(filename:&Path) -> Self {
		let conn = Connection::open(filename).expect("Unable to open DB file.");

		// Initialize our image DB and our indices.
		conn.execute(IMAGE_SCHEMA_V1, params![]).unwrap();
		conn.execute("CREATE TABLE watched_directories (glob TEXT PRIMARY KEY)", NO_PARAMS).unwrap();
		conn.execute("CREATE TABLE phash (id INTEGER PRIMARY KEY, hash BLOB)", params![]).unwrap();
		make_phash_distance_db_function(&conn);
		if let Err((_, e)) = conn.close() {
			eprintln!("Failed to close db after table creation: {}", e);
		}

		Engine::open(filename)
	}

	pub fn open(filename:&Path) -> Self {
		let manager = SqliteConnectionManager::file(filename);
		let pool = r2d2::Pool::new(manager).unwrap();

		Engine {
			pool,
			crawler_items: None
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
		// If we are already running a job, noop.

		// Select all our monitored folders and, in parallel, dir walk them to grab new images.
		let conn = self.pool.get().unwrap();
		let mut stmt = conn.prepare("SELECT glob FROM watched_directories").unwrap();
		let glob_cursor = stmt.query_map(NO_PARAMS, |row|{
			let dir:String = row.get(0)?;
			Ok(dir)
		}).unwrap();

		let all_globs:Vec<String> = glob_cursor.map(|item|{
			item.unwrap()
		}).collect();

		// Spawn one thread to crawl the disk and a few others to parallel process images.
		let (s, r): (crossbeam::channel::Sender<PathBuf>, crossbeam::channel::Receiver<PathBuf>) = crossbeam::channel::bounded(MAX_PENDING_FILEPAHTS);

		// Image Processing Thread.
		for _ in 0..4 {
			let pool = self.pool.clone();
			let rx = r.clone();
			std::thread::spawn(move || {
				println!("Procesor reporting for duty.");
				let conn = pool.get().unwrap();
				while let Ok(image_path) = rx.recv() {
					println!("Processing {}", &image_path.display());
					// Calculate everything that needs calculating and insert it.
					match IndexedImage::from_file_path(&image_path.as_path()) {
						Ok(img) => {
							if let Err(e) = Engine::insert_image(&conn, img) {
								eprintln!("Failed to track image {}: {}", &image_path.display(), &e);
							} else {
								println!("Success.");
							}
						},
						Err(e) => {
							println!("Error processing {}: {}", image_path.display(), e);
						}
					}
				}
				conn.flush_prepared_statement_cache();
			});
		}

		// Crawling Thread.
		{
			let pool = self.pool.clone();
			let conn = pool.get().unwrap();
			std::thread::spawn(move || {
				println!("Crawler reporting for duty.");
				let mut stmt = conn.prepare("SELECT 1 FROM images WHERE path = ?").unwrap();
				for g in all_globs {
					for maybe_fname in glob(&g).expect("Failed to interpret glob pattern.") {
						match maybe_fname {
							Ok(path) => {
								if path.is_file() && Engine::is_supported_extension(&path) && !stmt.exists(params![path.canonicalize().unwrap().display().to_string()]).unwrap() {
									if let Err(e) = s.send(path) {
										eprintln!("Failed to submit image for processing: {}", e);
									}
								}
							},
							Err(e) => eprintln!("Failed to match glob: {}", e)
						}
					}
				}
				drop(s);
			});
		}
	}

	//fn get_reindexing_status(&self) -> bool {}

	fn insert_image(conn: &PooledConnection<SqliteConnectionManager>, img:IndexedImage) -> Result<()> {
		// Update the images table first...
		conn.execute(
			"INSERT INTO images (filename, path, thumbnail) VALUES (?, ?, ?)",
			params![img.filename, img.path, img.thumbnail]
		)?;

		// Add the hashes.
		conn.execute(
			"INSERT INTO phashes (id, hash) VALUES (?, ?)",
			params![img.id, img.phash.unwrap()]
		)?;

		Ok(())
	}

	pub fn add_tracked_folder(&mut self, folder_glob:String) {
		self.pool.get().unwrap().execute("INSERT INTO watched_directories (glob) VALUES (?1)", params![folder_glob]).unwrap();
	}

	pub fn remove_tracked_folder(&mut self, folder_glob:String) {
		self.pool.get().unwrap().execute("DELETE FROM watched_directories WHERE glob=?1", params![folder_glob]).unwrap();
	}

	fn is_supported_extension(path:&PathBuf) -> bool {
		if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
			let ext = extension.to_lowercase();
			for &supported_extension in &["png", "bmp", "jpg", "jpeg", "gif"] {
				if ext == supported_extension {
					return true;
				}
			}
		}
		return false;
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

fn make_phash_distance_db_function(db: &Connection) -> Result<()> {
	db.create_scalar_function(
		"phash_distance",
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