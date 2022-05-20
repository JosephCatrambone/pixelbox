use anyhow::Result;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, BufRead};
use std::time::Instant;
use std::path::Path;
//use exif::{Field, Exif, };
use image::{ImageError, GenericImageView, DynamicImage};

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
	pub created: Instant,
	pub indexed: Instant,

	pub tags: HashMap<String, String>,

	pub phash: Option<Vec<u8>>,
	pub visual_hash: Option<Vec<u8>>, // For visual-similarity, like style and structure.  Not for content.
	//pub content_hash: Option<Vec<u8>>, //

	pub distance_from_query: Option<f64>,
}

impl IndexedImage {
	pub fn from_file_path(path:&Path) -> Result<Self> {
		let mut file = File::open(path)?;
		let mut bytes = vec![];
		let _bytes_read = file.read(bytes.as_mut_slice())?;
		//let mut img = image::io::Reader::new(&mut image_buffer).decode()?;

		let filename:String = path.file_name().unwrap().to_str().unwrap().to_string();
		let pathstring:String = stringify_filepath(path);

		IndexedImage::from_memory(&mut bytes, filename, pathstring)
	}

	pub fn from_memory(bytes:&mut Vec<u8>, filename:String, path:String) -> Result<Self> {
		//let mut img = image::open(path)?;
		let mut img:DynamicImage = image::load_from_memory(bytes)?;
		let thumb = img.thumbnail(THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1).to_rgb8();
		let thumbnail_width = thumb.width();
		let thumbnail_height = thumb.height();
		let qoi_thumb = qoi::encode_to_vec(&thumb.into_raw(), thumbnail_width, thumbnail_height).expect("Unable to generate compressed thumbnail.");

		// Also parse the EXIF data.
		let mut bufread = Cursor::new(bytes);
		let mut tags = HashMap::<String, String>::new();
		let mut exifreader = exif::Reader::new();
		if let Ok(exif) = exifreader.read_from_container(&mut bufread) {
			for field in exif.fields() {
				tags.insert(field.tag.to_string(), field.display_value().to_string());
			}
		}

		// And generate a perceptual hash.
		let hash = Some(mlhash(&img));

		Ok(
			IndexedImage {
				id: 0,
				filename: filename,
				path: path,
				resolution: (img.width(), img.height()),
				thumbnail: qoi_thumb,
				created: Instant::now(),
				indexed: Instant::now(),

				tags: tags,

				phash: None, // Some(phash(&img)),  // Disable for a little while to check performance.
				visual_hash: hash,

				distance_from_query: None,
			}
		)
	}

	pub fn get_thumbnail(&self) -> (Vec<u8>, (u32, u32)) {
		let (header, data) = qoi::decode_to_vec(&self.thumbnail).expect("Failed to decode thumbnail.");
		(data, (header.width, header.height))
	}
}

/// Convert a path into a canonical string.
/// We could do a few different things to a path, but to ensure we're doing the same thing everywhere we reference a path as a string, have one method.
pub fn stringify_filepath(path: &Path) -> String {
	path.canonicalize().unwrap().display().to_string()
}

#[cfg(test)]
mod tests {
	// Note this useful idiom: importing names from outer (for mod tests) scope.
	use super::*;

	#[test]
	fn test_load_resource() {
		let img = IndexedImage::from_file_path(Path::new("test_resources/flat_white.png"));
		//assert_eq!(add(1, 2), 3);
	}
}
