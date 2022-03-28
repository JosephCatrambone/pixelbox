use std::io::Cursor;
use image::{DynamicImage, GenericImageView};
use lazy_static::lazy_static;
use tract_ndarray::Array;
use tract_onnx::prelude::*;

const ENCODER_MODEL_PATH:&'static str = "models/encoder_cpu.onnx";
const STYLE_ENCODER_MODEL_PATH:&'static str = "models/style_encoder_cpu.onnx";
const MODEL_INPUT_WIDTH:usize = 255;
const MODEL_INPUT_HEIGHT:usize = 255;
const MODEL_LATENT_SIZE:usize = 128;

//static ref ENCODER_MODEL:tch::CModule = tch::CModule::load(ENCODER_MODEL_PATH).expect("Failed to find models at expected location: models/traced_*coder_cpu.pt");
lazy_static! {
	static ref MODEL:SimplePlan<TypedFact, Box<dyn TypedOp>, tract_onnx::prelude::Graph<TypedFact, Box<dyn TypedOp>>> =
		tract_onnx::onnx()
		// load the model
		.model_for_path(ENCODER_MODEL_PATH)
		.expect("Failed to load model from models/encoder_cpu.onnx")
		// specify input type and shape
		.with_input_fact(0, InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, MODEL_INPUT_HEIGHT as i64, MODEL_INPUT_WIDTH as i64)))
		.expect("Failed to specify input shape.")
		// optimize the model
		.into_optimized()
		.expect("Failed to optimize model.")
		// make the model runnable and fix its inputs and outputs
		.into_runnable()
		.expect("Failed make model runnable.");

	static ref STYLE_MODEL:SimplePlan<TypedFact, Box<dyn TypedOp>, tract_onnx::prelude::Graph<TypedFact, Box<dyn TypedOp>>> =
		tract_onnx::onnx()
		// load the model
		.model_for_path(STYLE_ENCODER_MODEL_PATH)
		.expect(format!("Failed to load model from {}", STYLE_ENCODER_MODEL_PATH))
		// specify input type and shape
		.with_input_fact(0, InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, MODEL_INPUT_HEIGHT as i64, MODEL_INPUT_WIDTH as i64)))
		.expect("Failed to specify input shape.")
		// optimize the model
		.into_optimized()
		.expect("Failed to optimize model.")
		// make the model runnable and fix its inputs and outputs
		.into_runnable()
		.expect("Failed make model runnable.");
}

pub fn mlhash(img:&DynamicImage) -> Vec<u8> {
	hash(img, &MODEL)
}


pub fn style_hash(img:&DynamicImage) -> Vec<u8> {
	hash(img, &STYLE_MODEL)
}

fn hash(img:&DynamicImage, model:&SimplePlan<TypedFact, Box<dyn TypedOp>, tract_onnx::prelude::Graph<TypedFact, Box<dyn TypedOp>>>) -> Vec<u8> {
	let img = img.to_rgb8();
	let resized = image::imageops::resize(&img, MODEL_INPUT_WIDTH as u32, MODEL_INPUT_HEIGHT as u32, ::image::imageops::FilterType::Triangle);
	//let mean = Array::from_shape_vec((1, 3, 1, 1), vec![0.485, 0.456, 0.406])?;
	let image: Tensor =
		tract_ndarray::Array4::from_shape_fn((1, 3, MODEL_INPUT_HEIGHT, MODEL_INPUT_WIDTH), |(_, c, y, x)| {
			resized[(x as _, y as _)][c] as f32 / 255.0
		}).into();

	let result = model.run(tvec!(image)).unwrap();

	// find and display the max value with its index
	result[0]
		.to_array_view::<f32>().unwrap()
		.iter()
		.map(|v|{ (128f32 + (v.max(-1f32).min(1f32) * 128f32)) as u8 })
		.collect()
}