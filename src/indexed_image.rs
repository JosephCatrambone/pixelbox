use std::time::Instant;
use std::path::Path;
use image::ImageError;

use crate::image_hashes::phash;

pub const THUMBNAIL_SIZE: (u32, u32) = (256, 256);

#[derive(Clone, Debug)]
pub struct IndexedImage {
	pub id: i64,
	pub filename: String,
	pub path: String,
	pub thumbnail: Vec<u8>,
	pub created: Instant,
	pub indexed: Instant,

	pub phash: Option<Vec<u8>>,
	pub semantic_hash: Option<Vec<u8>>,

}

impl IndexedImage {
	pub fn from_file_path(path:&Path) -> Result<Self, ImageError> {
		let mut img = image::open(path)?;
		let thumb = img.thumbnail(THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1).to_rgb();

		Ok(
			IndexedImage {
				id: 0,
				filename: path.file_name().unwrap().to_str().unwrap().to_string(),
				path: stringify_filepath(path),
				thumbnail: thumb.to_vec(),
				created: Instant::now(),
				indexed: Instant::now(),

				phash: Some(phash(&img)),
				semantic_hash: None
			}
		)
	}
}

/// Convert a path into a canonical string.
/// We could do a few different things to a path, but to ensure we're doing the same thing everywhere we reference a path as a string, have one method.
pub fn stringify_filepath(path: &Path) -> String {
	path.canonicalize().unwrap().display().to_string()
}

