use rusqlite::{params, Connection, Result};
use std::path::Path;
use image::DynamicImage;

struct Engine {
	connection: Connection,
	tracked_folders: Vec<String>,
}

#[derive(Debug)]
struct IndexedImage {
	id: i32,
	filename: String,
	data: Option<Vec<u8>>,
}

impl Engine {
	fn new() -> Result<()> {
		let conn = Connection::open_in_memory()?;

		conn.execute(
			"CREATE TABLE images (
			id              INTEGER PRIMARY KEY,
			filename        TEXT NOT NULL,
			data            BLOB
		)",
			[],
		)?;
		let me = IndexedImage {
			id: 0,
			filename: "steven.png".to_string(),
			data: None,
		};
		conn.execute(
			"INSERT INTO person (filename, data) VALUES (?1, ?2)",
			params![me.name, me.data],
		)?;

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
		Ok(())
	}

	fn start_reindexing(&mut self) {

	}

	//fn get_reindexing_status(&self) -> bool {}

	fn add_tracked_folder(&mut self, folder_glob:String) {

	}

	fn remove_tracked_folder(&mut self, folder_index:usize) {

	}

	fn index_image_from_filename(&mut self, filename:String) {

	}

	fn index_image_from_path(&mut self, file:Path) {

	}

	fn index_image(&mut self, img:DynamicImage) {
		
	}
}