use std::time::Instant;
use std::path::Path;
use std::error::Error;

#[derive(Debug)]
pub struct IndexedImage {
	// Make sure the create table matches up with this.
	pub id: i32,
	pub filename: String,
	pub path: String,
	pub thumbnail: Vec<u8>,
	pub crypto_hash: Vec<u8>,
	pub phash: Vec<u8>,
	pub semantic_hash: Vec<u8>,
	pub created: Instant,
	pub indexed: Instant,
	//data: Option<Vec<u8>>,
	pub text: String,  // Text that appears in an image.
}

impl IndexedImage {
	pub fn make_table_sql() -> &str {
		"CREATE TABLE images (
			id             INTEGER PRIMARY KEY,
			filename       TEXT NOT NULL,
			path           TEXT NOT NULL,
			thumbnail      BLOB,
			crypto_hash    BLOB,
			phash          BLOB,
			semantic_hash  BLOB,
			created        DATETIME,
			indexed        DATETIME,
			text           TEXT NOT NULL
		)"
	}

	pub fn from_file_path(file:Path) -> Result<Self, Error> {
		let mut img = image::open(file)?;
	}
}

