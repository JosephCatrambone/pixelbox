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
	crawled_images: Vec<IndexedImage>,
}

impl Crawler {
	pub fn new() -> Self {
		Crawler {
			crawled_images: Vec::new(),
		}
	}

	pub fn start_indexing(&mut self, folders: Vec<String>) -> Receiver<IndexedImage> {
		let (image_tx, image_rx) = unbounded();

		let res = image_rx;

		{
			let globs = folders.clone();
			let tx = image_tx.clone();

			std::thread::spawn(move || {
				'end: for mut g in globs {
					g.push(std::path::MAIN_SEPARATOR);
					g.push_str("**");
					g.push(std::path::MAIN_SEPARATOR);
					g.push_str("*.*");
					for maybe_fname in glob(&g).expect("Failed to interpret glob pattern.") {
						match maybe_fname {
							Ok(path) => {
								println!("Crawling {} ...", stringify_filepath(&path));
								if path.is_file() {
									if let Some(extension) = path.extension().and_then(OsStr::to_str) {
										let mut is_image_file = false;
										for &ext in SUPPORTED_IMAGE_EXTENSIONS {
											if extension.eq_ignore_ascii_case(ext) {
												is_image_file = true;
											}
										}

										if is_image_file {
											match IndexedImage::from_file_path(&path.as_path()) {
												Ok(img) => {
													let e = tx.send(img);
													if e.is_err() {
														// Close our connection.
														break 'end;
													}
													println!("... processed!");
												},
												Err(e) => {
													println!("Error processing {}: {}", path.display(), e);
												}
											}
										} else {
											println!("... skipped");
										}
									} // Else we have to skip it.  No extension.
								}
							},
							Err(e) => eprintln!("Failed to match glob: {}", e)
						}
					}
				}
				drop(tx);
			});
		}

		res
	}
}
