///
/// engine.rs
/// Handles indexing and keeping track of active searches.
/// Closely tied to indexed_image, but indexed_image has a bunch of extra fields, like uncalculated hashes that aren't necessarily stored in the database.
/// Engine manages the spidering, indexing, and keeping track of images.
///

use glob::glob;
use image::DynamicImage;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rayon::prelude::*;
use rusqlite::{params, Connection, Error, Result, NO_PARAMS, MappedRows, Row};
use rusqlite::functions::FunctionFlags;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::indexed_image::*;

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;


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
}

impl Engine {
	pub fn new(filename:&Path) -> Self {
		let conn = Connection::open(filename).expect("Unable to open DB file.");

		// Initialize our image DB and our indices.
		conn.execute(IMAGE_SCHEMA_V1, params![]).unwrap();
		conn.execute("CREATE TABLE watched_directories (glob TEXT PRIMARY KEY)", NO_PARAMS).unwrap();
		conn.execute("CREATE TABLE phashes (id INTEGER PRIMARY KEY, hash BLOB)", params![]).unwrap();
		conn.close();

		Engine::open(filename)
	}

	pub fn open(filename:&Path) -> Self {
		let manager = SqliteConnectionManager::file(filename);
		let pool = r2d2::Pool::new(manager).unwrap();

		/*
		(0..10)
			.map(|i| {
				let pool = pool.clone();
				thread::spawn(move || {
					let conn = pool.get().unwrap();
					conn.execute("INSERT INTO foo (bar) VALUES (?)", &[&i])
						.unwrap();
				})
			})
			.collect::<Vec<_>>()
			.into_iter()
			.map(thread::JoinHandle::join)
			.collect::<Result<_, _>>()
			.unwrap()
		*/

		Engine {
			pool,
		}
	}

	pub fn shutdown(&mut self) {
	}

	pub fn start_reindexing(&mut self) {
		// Select all our monitored folders and, in parallel, dir walk them to grab new images.
		let mut conn = self.pool.get().unwrap();
		let mut stmt = conn.prepare("SELECT glob FROM watched_directories").unwrap();
		let glob_cursor = stmt.query_map(NO_PARAMS, |row|{
			let dir:String = row.get(0)?;
			Ok(dir)
		}).unwrap();

		let all_globs:Vec<String> = glob_cursor.map(|item|{
			item.unwrap()
		}).collect();

		// Spawn one thread to read all the directories on disk and then use Rayon to parallel
		let (s, r): (crossbeam::channel::Sender<PathBuf>, crossbeam::channel::Receiver<PathBuf>) = crossbeam::channel::unbounded();
		std::thread::spawn(move||{
			while let Ok(image_path) = r.recv() {
				println!("Parsing and indexing {}", image_path.display());
			}
		});

		std::thread::spawn(move||{
			for g in all_globs {
				for maybe_fname in glob(&g).expect("Failed to interpret glob pattern.") {
					match maybe_fname {
						Ok(path) => {
							if path.is_file() {
								s.send(path);
							}
						},
						Err(e) => eprintln!("Failed to match glob: {}", e)
					}
				}
			}
			drop(s);
		});
	}

	//fn get_reindexing_status(&self) -> bool {}

	pub fn add_tracked_folder(&mut self, folder_glob:String) {
		self.pool.get().unwrap().execute("INSERT INTO watched_directories (glob) VALUES (?1)", params![folder_glob]).unwrap();
	}

	pub fn remove_tracked_folder(&mut self, folder_glob:String) {
		self.pool.get().unwrap().execute("DELETE FROM watched_directories WHERE glob=?1", params![folder_glob]).unwrap();
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