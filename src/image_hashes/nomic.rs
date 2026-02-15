use std::sync::LazyLock;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use tract_onnx::prelude::*;
use tract_onnx::prelude::DatumType::F32;
use tract_onnx::tract_hir::infer::InferenceOp;

const SIMILARITY_MODEL_PATH:&'static str = "models/nomic_embed_vision_v1_5_int8.onnx";
const MODEL_INPUT_WIDTH:u32 = 224;
const MODEL_INPUT_HEIGHT:u32 = 224;
const MODEL_LATENT_SIZE:usize = 197*768;


static MODEL: LazyLock<RunnableModel<InferenceFact, Box<dyn InferenceOp>, Graph<InferenceFact, Box<dyn InferenceOp>>>> = LazyLock::new(|| {
//static MODEL: LazyLock<RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>> = LazyLock::new(|| {
	let mut m = tract_onnx::onnx()
		.model_for_path(SIMILARITY_MODEL_PATH).expect("Unable to load similarity model from disk!")
		.with_input_fact(0, f32::fact([1, 3, 224, 224]).into()).expect("Could not set input fact.")
		//.with_output_fact(0, f32::fact([1, 197, 768]).into()).expect("Could not set output fact.")
		.with_output_fact(0, f32::fact([1, 3, 14, 16, 14, 16]).into()).expect("Could not set output fact.")
		//.into_optimized().expect("Model optimization step failed.")
		.into_runnable().expect("Model runnable conversion failed.");
	m
});


fn image_to_tensor(img: &DynamicImage) -> Tensor {
	let img = img.resize_to_fill(MODEL_INPUT_WIDTH, MODEL_INPUT_HEIGHT, FilterType::Triangle).to_rgb8();
	let data: Tensor = tract_ndarray::Array4::from_shape_fn((1, 3, 224, 224), |(_, c, y, x)| {
		let mean = 0.0;
		let std = 1.0;
		(img[(x as _, y as _)][c] as f32 / 255.0 - mean) / std
	}).into();
	data
}

pub fn mlhash(img:&DynamicImage) -> Vec<u8> {
	//let model = tract_onnx::onnx().model_for_path(SIMILARITY_MODEL_PATH).expect("Unable to load similarity model from disk!").into_optimized().unwrap().into_runnable().unwrap();
	let img_tensor = image_to_tensor(img);

	let output = MODEL.run(tvec!(img_tensor.into())).expect("Failed to generate embedding for image. This should never happen.");
	let float_embed = output[0]
		.to_array_view::<f32>()
		.unwrap()
		.iter()
		.map(|f| { 128u8.saturating_add_signed((f*128.0f32).max(-128.0f32).min(128.0f32) as i8) })
		.collect::<Vec<u8>>();
	float_embed
}

#[cfg(test)]
mod test {
	use std::env;
	use std::path::Path;
	use crate::engine::hamming_distance;
	use super::mlhash;

	const SRC_FILE: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/", file!());
	const TEST_IMAGE_DIRECTORY: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/", "test_resources");

	#[test]
	fn test_sanity() {
		println!("CWD: {:?}", &env::current_dir().unwrap());
		println!("Loading images from {:}", TEST_IMAGE_DIRECTORY);

		let mut diff = 0f32;
		let img = image::open(Path::new(TEST_IMAGE_DIRECTORY).join("phash_test_a.png")).unwrap();
		let img_hash = mlhash(&img);

		// Cases that should match:
		diff = hamming_distance(&img_hash, &img_hash);
		assert_eq!(diff, 0f32);
		//assert!(hamming_distance(&flat_hash, &img_rot_hash) > 0.5);
	}
}
