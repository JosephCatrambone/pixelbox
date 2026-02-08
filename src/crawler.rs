use anyhow::{Result, anyhow};
use crossbeam::channel::{Receiver, Sender, unbounded};
use glob::glob;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufReader, BufRead, Read};
use std::path::PathBuf;

use crate::indexed_image::{IndexedImage, stringify_filepath};

const SUPPORTED_IMAGE_EXTENSIONS: &'static [&str; 12] = &["png", "bmp", "jpg", "jpeg", "jfif", "gif", "tiff", "pnm", "webp", "ico", "tga", "exr"];


#[derive(Default)]
pub struct Crawler {
	tracked_folders: Vec<String>,
	on_filename_crawled: Vec<Box<dyn Fn(&String)>>,
	on_image_processed: Vec<Box<dyn Fn(IndexedImage)>>,
	on_image_processing_failure: Vec<Box<dyn Fn(&String, &String)>>,
}

impl Crawler {
	pub fn new() -> Self {
		Crawler::default()
	}
	
	pub fn add_tracked_folder(&mut self, folder: String) {
		
	}
	
	pub fn rescan_index(&mut self, force: bool) {
		
	}
}

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
							if path.is_file() {
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
			while let Ok(file_path) = rx.recv() {
				// File path is any generic file, not necessarily an image file.
				// We need to check if it's an image, a zip file, or something else.
				if let Some(extension) = file_path.extension().and_then(OsStr::to_str) {
					let mut is_image_file = false;
					for &ext in SUPPORTED_IMAGE_EXTENSIONS {
						if extension.eq_ignore_ascii_case(ext) {
							is_image_file = true;
						}
					}

					if is_image_file {
						match IndexedImage::from_file_path(&file_path.as_path()) {
							Ok(img) => {
								tx.send(img);
							},
							Err(e) => {
								println!("Error processing {}: {}", file_path.display(), e);
							}
						}
					}
				} // Else we have to skip it.  No extension.
			}
		});
	}

	(file_rx, image_rx)
}