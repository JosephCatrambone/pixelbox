
use crossbeam::channel::{Receiver, Sender, unbounded};
use glob::glob;
use std::path::PathBuf;

use crate::indexed_image::{IndexedImage, stringify_filepath};

/// Given a vec of directory globs and a set of valid extensions,
/// crawl the disk and index images.
/// Returns a Channel with Images as they're created.
pub fn crawl_globs_async(globs:Vec<String>, parallel_file_loaders:usize) -> (Receiver<PathBuf>, Receiver<IndexedImage>) {

	let (file_tx, file_rx) = unbounded();
	let (image_tx, image_rx) = unbounded();

	// TODO: A bloom filter to make sure we don't reprocess any images we have already.

	// Crawling Thread.
	{
		let tx = file_tx.clone();
		std::thread::spawn(move || {
			println!("Crawler reporting for duty.");
			for mut g in globs {
				g.push(std::path::MAIN_SEPARATOR);
				g.push_str("**");
				g.push(std::path::MAIN_SEPARATOR);
				g.push_str("*.*");
				for maybe_fname in glob(&g).expect("Failed to interpret glob pattern.") {
					match maybe_fname {
						Ok(path) => {
							println!("Checking {}", stringify_filepath(&path));
							if path.is_file() && is_supported_extension(&path) {
								if let Err(e) = tx.send(path) {
									eprintln!("Failed to submit image for processing: {}", e);
								}
							}
						},
						Err(e) => eprintln!("Failed to match glob: {}", e)
					}
				}
			}
			drop(tx);
		});
	}

	// Image Processing Thread.
	for _ in 0..parallel_file_loaders {
		let rx = file_rx.clone();
		let tx = image_tx.clone();
		std::thread::spawn(move || {
			while let Ok(image_path) = rx.recv() {
				// Calculate the bare minimum that needs calculating and insert it.
				match IndexedImage::from_file_path(&image_path.as_path()) {
					Ok(img) => {
						tx.send(img);
					},
					Err(e) => {
						println!("Error processing {}: {}", image_path.display(), e);
					}
				}
			}
		});
	}

	(file_rx, image_rx)
}

fn is_supported_extension(path:&PathBuf) -> bool {
	if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
		let ext = extension.to_lowercase();
		for &supported_extension in &["png", "bmp", "jpg", "jpeg", "gif", "tiff", "pnm", "webp", "ico", "tga", "exr"] {
			if ext == supported_extension {
				return true;
			}
		}
	}
	return false;
}