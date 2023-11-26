use image::{DynamicImage, imageops};

pub fn phash(img:&DynamicImage) -> Vec<u8> {
	// Each pixel becomes one bit.  16x16 pixels = 256 bits = 32 bytes
	let img_width = 16;
	let img_height = 16;
	let small = img.resize(img_width, img_height, image::imageops::Gaussian);
	let grey = imageops::grayscale(&small).to_vec();
	let total_hash_bytes = grey.len() / 8;
	let mean = (grey.iter().map(|&x|{ x as u64 }).sum::<u64>() / ((img_width*img_height) as u64)) as u8;
	let bytes: Vec<u8> = (0..total_hash_bytes).into_iter().map(|byte_idx|{
		// Make these eight bits in grey into a byte.
		let mut byte_accumulator = 0u8;
		for i in 0..8 {
			if grey[8*byte_idx + i] > mean {
				byte_accumulator |= 1 << i;
			}
		}
		byte_accumulator
	}).collect();
	bytes
}

#[cfg(test)]
mod test {
	use criterion;
	use image;
	use std::env;
	use std::path::Path;
	use crate::engine::hamming_distance;
	use crate::image_hashes::phash::*;

	const SRC_FILE: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/", file!());
	const TEST_IMAGE_DIRECTORY: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/", "test_resources");

	#[test]
	fn test_phash_flat_white() {
		let img = image::open(Path::new(TEST_IMAGE_DIRECTORY).join("flat_white.png")).unwrap();
		let hash = phash(&img);
		assert_eq!(hash, vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
	}

	#[test]
	fn test_phash() {
		println!("CWD: {:?}", &env::current_dir().unwrap());
		println!("Loading images from {:}", TEST_IMAGE_DIRECTORY);

		let mut diff = 0f32;
		let img = image::open(Path::new(TEST_IMAGE_DIRECTORY).join("phash_test_a.png")).unwrap();
		let img_hash = phash(&img);

		// Cases that should match:
		diff = hamming_distance(&img_hash, &img_hash);
		assert_eq!(diff, 0f32);

		let img_resize = image::open(Path::new(TEST_IMAGE_DIRECTORY).join("phash_test_resize.png")).unwrap();
		let img_resize_hash = phash(&img_resize);
		diff = hamming_distance(&img_hash, &img_resize_hash);
		assert!(diff < 0.0001);

		let img_crop = image::open(Path::new(TEST_IMAGE_DIRECTORY).join("phash_test_crop.png")).unwrap();
		let img_crop_hash = phash(&img_crop);
		diff = hamming_distance(&img_hash, &img_crop_hash);
		assert!(diff < 0.5);

		let img_rot = image::open(Path::new(TEST_IMAGE_DIRECTORY).join("phash_test_rot1.png")).unwrap();
		let img_rot_hash = phash(&img_rot);
		diff = hamming_distance(&img_hash, &img_rot_hash);
		assert!(diff < 0.5);

		// Cases that should be different.
		let flat = image::open(Path::new(TEST_IMAGE_DIRECTORY).join("flat_white.png")).unwrap();
		let flat_hash = phash(&flat);
		assert!(hamming_distance(&flat_hash, &img_hash) > 0.5);
		assert!(hamming_distance(&flat_hash, &img_resize_hash) > 0.5);
		assert!(hamming_distance(&flat_hash, &img_crop_hash) > 0.5);
		assert!(hamming_distance(&flat_hash, &img_rot_hash) > 0.5);
	}
	
	//#[bench]
	fn bench_phash(b: &mut criterion::Criterion) {
		let img = image::open("test_resources/flat_white.png").unwrap();

		b.bench_function("plain_phash_256x256", move |bencher|{
			phash(&img);
		});
	}
}