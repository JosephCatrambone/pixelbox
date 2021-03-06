use std::time::Instant;
use std::path::Path;
use image::ImageError;

pub const THUMBNAIL_SIZE: (u32, u32) = (256, 256);

#[derive(Debug)]
pub struct IndexedImage {
	pub id: i32,
	pub filename: String,
	pub path: String,
	pub thumbnail: Vec<u8>,
	pub created: Instant,
	pub indexed: Instant,

	phash: Option<Vec<u8>>,
	semantic_hash: Option<Vec<u8>>,

}

impl IndexedImage {
	pub fn from_file_path(file:&Path) -> Result<Self, ImageError> {
		let mut img = image::open(file)?;
		todo!()
	}
}

