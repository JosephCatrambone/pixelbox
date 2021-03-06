use image::DynamicImage;
use rusqlite::{params, Connection, Result};
use std::path::Path;

use crate::indexed_image::IndexedImage;
use std::time::Instant;

const THUMBNAIL_SIZE: (u32, u32) = (256, 256);

struct Engine {
	connection: Connection,
	tracked_folders: Vec<String>,
}

impl Engine {
	fn new() -> Result<Self> {
		let conn = Connection::open_in_memory()?;

		conn.execute(IndexedImage::make_table_sql(), [])?;

		/*
		let mut stmt = conn.prepare("SELECT id, name, data FROM person")?;
		let img_iter = stmt.query_map([], |row| {
			Ok(IndexedImage {
				id: row.get(0)?,
				filename: row.get(1)?,
				data: row.get(2)?,
			})
		})?;

		for person in img_iter {
			println!("Found person {:?}", person.unwrap());
		}
		*/
		Ok(
			Engine {
				connection: conn,
				tracked_folders: vec![]
			}
		)
	}

	fn start_reindexing(&mut self) {

	}

	//fn get_reindexing_status(&self) -> bool {}

	fn add_tracked_folder(&mut self, folder_glob:String) {
		self.tracked_folders.push(folder_glob);
	}

	fn remove_tracked_folder(&mut self, folder_index:usize) {
		self.tracked_folders.remove(folder_index);
	}

	fn index_image_from_path(&mut self, file:Path) {
		let indexed_image = IndexedImage::from_file_path(file).expect("TODO: Handle failure");
		self.index_image(img);
	}

	fn index_image(&mut self, img:IndexedImage) {
		self.connection.execute(
			"INSERT INTO images (filename, path, thumbnail, crypto_hash, phash, semantic_hash, created, indexed, text) VALUES (?1, ?2)",
			params![img.filename, img.path, img.thumbnail, img.crypto_hash, img.phash, img.semantic_hash, img.created, Instant::now(), img.text]
		);
	}
}