use image::DynamicImage;

fn phash(img:&DynamicImage) -> Vec<u8> {
	// Each pixel becomes one bit.  16x16 pixels = 256 bits = 32 bytes
	let img_width = 16;
	let img_height = 16;
	let small = img.resize(img_width, img_height, image::imageops::Gaussian);
	let grey = small.to_luma().to_vec();
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
	use image;
	use crate::engine::hamming_distance;
	use crate::image_hashes::phash::*;

	#[test]
	fn test_phash_flat_white() {
		let img = image::open("test_resources/flat_white.png").unwrap();
		let hash = phash(&img);
		assert_eq!(hash, vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
	}

	#[test]
	fn test_phash() {
		let mut diff = 0f32;
		let img = image::open("test_resources/phash_test_a.png").unwrap();
		let img_hash = phash(&img);

		// Cases that should match:
		diff = hamming_distance(&img_hash, &img_hash);
		assert_eq!(diff, 0f32);

		let img_resize = image::open("test_resources/phash_test_resize.png").unwrap();
		let img_resize_hash = phash(&img_resize);
		diff = hamming_distance(&img_hash, &img_resize_hash);
		assert!(diff < 0.0001);

		let img_crop = image::open("test_resources/phash_test_crop.png").unwrap();
		let img_crop_hash = phash(&img_crop);
		diff = hamming_distance(&img_hash, &img_crop_hash);
		assert!(diff < 0.5);

		let img_rot = image::open("test_resources/phash_test_rot1.png").unwrap();
		let img_rot_hash = phash(&img_rot);
		diff = hamming_distance(&img_hash, &img_rot_hash);
		assert!(diff < 0.5);

		// Cases that should be different.
		let flat = image::open("test_resources/flat_white.png").unwrap();
		let flat_hash = phash(&flat);
		assert!(hamming_distance(&flat_hash, &img_hash) > 0.5);
		assert!(hamming_distance(&flat_hash, &img_resize_hash) > 0.5);
		assert!(hamming_distance(&flat_hash, &img_crop_hash) > 0.5);
		assert!(hamming_distance(&flat_hash, &img_rot_hash) > 0.5);
	}
}