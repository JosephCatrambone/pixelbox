use criterion::{black_box, criterion_group, criterion_main, Criterion};
use image::{DynamicImage, Pixel, Rgb};
use image::RgbImage;

use pixelbox::image_hashes::phash::phash;

fn fibonacci(n: u64) -> u64 {
	match n {
		0 => 1,
		1 => 1,
		n => fibonacci(n-1) + fibonacci(n-2),
	}
}

fn criterion_benchmark(c: &mut Criterion) {
	c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
}

fn benchmark_phash_function(c: &mut Criterion) {
	for img_size in [128, 128*2, 128*3, 128*4, 128*5, 128*6, 128*7, 128*8] {
		let img: DynamicImage = DynamicImage::ImageRgb8(RgbImage::from_fn(img_size, img_size, |x, y|{
			Rgb::from_channels(0, x as u8 % 255, y as u8 % 255, 0)
		}));
		
		c.bench_function(&format!("phash size {}", img_size), |bench| {
			bench.iter(||{
				phash(&img);
			});
		});
	}
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);