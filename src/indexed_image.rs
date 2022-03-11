use std::time::Instant;
use std::path::Path;
use image::{ImageError, GenericImageView};

use crate::image_hashes::phash;
use crate::image_hashes::mlhash;
//use crate::image_hashes::efficientnet_hash;

pub const THUMBNAIL_SIZE: (u32, u32) = (256, 256);

#[derive(Clone, Debug)]
pub struct IndexedImage {
	pub id: i64,
	pub filename: String,
	pub path: String,
	pub resolution: (u32, u32),
	pub thumbnail: Vec<u8>,
	pub thumbnail_resolution: (u32, u32),
	pub created: Instant,
	pub indexed: Instant,

	pub phash: Option<Vec<u8>>,
	pub visual_hash: Option<Vec<u8>>, // For visual-similarity, like style and structure.  Not for content.
	//pub content_hash: Option<Vec<u8>>, //

	pub distance_from_query: Option<f64>,
}

impl IndexedImage {
	pub fn from_file_path(path:&Path) -> Result<Self, ImageError> {
		let mut img = image::open(path)?;
		let thumb = img.thumbnail(THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1).to_rgb8();

		Ok(
			IndexedImage {
				id: 0,
				filename: path.file_name().unwrap().to_str().unwrap().to_string(),
				path: stringify_filepath(path),
				resolution: (img.width(), img.height()),
				thumbnail: thumb.to_vec(),
				thumbnail_resolution: (thumb.width(), thumb.height()),
				created: Instant::now(),
				indexed: Instant::now(),

				phash: None, // Some(phash(&img)),  // Disable for a little while to check performance.
				visual_hash: Some(mlhash(&img)),

				distance_from_query: None,
			}
		)
	}
}

/// Convert a path into a canonical string.
/// We could do a few different things to a path, but to ensure we're doing the same thing everywhere we reference a path as a string, have one method.
pub fn stringify_filepath(path: &Path) -> String {
	path.canonicalize().unwrap().display().to_string()
}

