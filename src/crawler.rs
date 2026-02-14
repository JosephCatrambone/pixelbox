use anyhow::{Result, anyhow};
use crossbeam::channel::{Receiver, bounded, unbounded, TryRecvError, TrySendError};
use glob::glob;
use std::ffi::OsStr;
use crate::indexed_image::{IndexedImage, stringify_filepath};

const SUPPORTED_IMAGE_EXTENSIONS: &'static [&str; 12] = &["png", "bmp", "jpg", "jpeg", "jfif", "gif", "tiff", "pnm", "webp", "ico", "tga", "exr"];
const MAX_PENDING_TX: usize = 128;

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

	pub fn start_indexing(
			&mut self,
			folders: Vec<String>,
			num_workers: usize,
	) -> Receiver<IndexedImage> {
		let (filename_tx, filename_rx) = unbounded();
		let (image_tx, image_rx) = bounded(MAX_PENDING_TX);

		let res = image_rx;

		{
			let globs = folders.clone();

			std::thread::spawn(move || {
				for mut g in globs {
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
											filename_tx.send(path).expect("Can't send file path to image processing thread. This should never happen.");
										}
									} // Else we have to skip it.  No extension.
								}
							},
							Err(e) => eprintln!("Failed to match glob: {}", e)
						}
					}
				}
				drop(filename_tx);
			});
		}

		for _ in 0..num_workers {
			let rx = filename_rx.clone();
			let tx = image_tx.clone();
			std::thread::spawn(move || {
				let mut send_buffer = Vec::new();
				let mut file_stream_open = true;
				'end: loop {
					if file_stream_open { // We might be able to avoid this check and just hit rx err every time.
						match rx.try_recv() {
							Ok(path) => {
								if let Ok(img) = IndexedImage::from_file_path(&path.as_path()) {
									send_buffer.push(img); // Build up buffer in the thread so we don't have as much IPC overhead.
								}
							},
							Err(TryRecvError::Empty) => {
								// Just wait.  Maybe sleep?
							},
							Err(TryRecvError::Disconnected) => {
								file_stream_open = false;
							}
						}
					}

					// Push the images back to the original thread.
					// If the image stream is closed, we can abort.
					if tx.is_full() { continue; }
					if let Some(img) = send_buffer.pop() {
						match tx.try_send(img.clone()) {
							Ok(_) => {},
							Err(TrySendError::Disconnected(_)) => {
								// Main thread can't get anything.
								return;
							},
							Err(TrySendError::Full(_)) => {
								// It's full.  Push back onto buffer.
								send_buffer.push(img);
								std::thread::yield_now();
							}
						}
					} else {
						std::thread::yield_now();
					}

					// Small chance that we just can't list file fast enough?
					if !file_stream_open && send_buffer.is_empty() {
						break 'end;
					}
				}
				drop(tx);
				println!("Image processing thread quitting. No work left.");
			});
		}

		res
	}
}
