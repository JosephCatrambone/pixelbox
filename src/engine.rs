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

		conn.execute(IndexedImage::make_table_sql(), params![])?;

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

	fn index_image_from_path(&mut self, file:&Path) {
		let img = IndexedImage::from_file_path(file).expect("TODO: Handle failure");
		self.index_image(img);
	}

	fn index_image(&mut self, img:IndexedImage) {
		self.connection.execute(
			"INSERT INTO images (filename, path, thumbnail) VALUES (?1, ?2, ?3, ?4, ?5)",
			params![img.filename, img.path, img.thumbnail] //, Instant::now().format("%Y-%m-%dT%H:%M:%S%.f").to_string()?
		);
	}
}