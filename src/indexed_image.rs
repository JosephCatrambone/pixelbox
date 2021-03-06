use std::time::Instant;
use std::path::Path;
use image::ImageError;

#[derive(Debug)]
pub struct IndexedImage {
	// Make sure the create table matches up with this.
	pub id: i32,
	pub filename: String,
	pub path: String,
	pub thumbnail: Vec<u8>,
	pub created: Instant,
	pub indexed: Instant,
	//data: Option<Vec<u8>>,
}

impl IndexedImage {
	pub fn make_table_sql() -> &'static str {
		"CREATE TABLE images (
			id             INTEGER PRIMARY KEY,
			filename       TEXT NOT NULL,
			path           TEXT NOT NULL,
			thumbnail      BLOB,
			created        DATETIME,
			indexed        DATETIME
		)"
	}

	pub fn from_file_path(file:&Path) -> Result<Self, ImageError> {
		let mut img = image::open(file)?;
		todo!()
	}
}

