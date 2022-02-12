
use image::{DynamicImage, GenericImageView};
use lazy_static::lazy_static;
use tract_ndarray::Array;
use tract_onnx::prelude::*;

const ENCODER_MODEL_PATH:&'static str = "models/efficientnet-lite4-11-int8-decapped.onnx";
const MODEL_INPUT_CHANNELS:usize = 3;
const MODEL_INPUT_WIDTH:usize = 224;
const MODEL_INPUT_HEIGHT:usize = 224;
const MODEL_LATENT_SIZE:usize = 1280;

lazy_static! {
	static ref MODEL:SimplePlan<TypedFact, Box<dyn TypedOp>, tract_onnx::prelude::Graph<TypedFact, Box<dyn TypedOp>>> =
		tract_onnx::onnx()
		// load the model
		.model_for_path(ENCODER_MODEL_PATH)
		.expect("Failed to load model from models/encoder_cpu.onnx")
		// specify input type and shape.  NOTE THAT UNLIKE OTHER MODELS, EFFICIENTNET IS CHANNELS-LAST!
		.with_input_fact(0, InferenceFact::dt_shape(f32::datum_type(), tvec!(1, MODEL_INPUT_HEIGHT as i64, MODEL_INPUT_WIDTH as i64, MODEL_INPUT_CHANNELS as i64)))
		.expect("Failed to specify input shape.")
		// optimize the model
		.into_optimized()
		.expect("Failed to optimize model.")
		// make the model runnable and fix its inputs and outputs
		.into_runnable()
		.expect("Failed make model runnable.");
}

pub fn efficientnet_hash(img:&DynamicImage) -> Vec<u8> {
	let img = img.to_rgb();
	let resized = image::imageops::resize(&img, MODEL_INPUT_WIDTH as u32, MODEL_INPUT_HEIGHT as u32, ::image::imageops::FilterType::Triangle);
	//let mean = Array::from_shape_vec((1, 3, 1, 1), vec![0.485, 0.456, 0.406])?;
	let image: Tensor =
		// Need to map from 0,255 to -1,1.
		tract_ndarray::Array4::from_shape_fn((1, MODEL_INPUT_HEIGHT, MODEL_INPUT_WIDTH, MODEL_INPUT_CHANNELS), |(_, y, x, c)| {
			(resized[(x as _, y as _)][c] as f32 / 128.0) - 1.0
		}).into();

	let result = MODEL.run(tvec!(image)).unwrap();

	// Model outputs are 1x1280 in u8.
	result[0]
		.to_array_view::<u8>().unwrap()
		.iter()
		.map(|v|{v.clone()})
		.collect()
}