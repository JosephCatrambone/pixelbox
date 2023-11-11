use std::io::Cursor;
use image::{DynamicImage, GenericImageView};
use lazy_static::lazy_static;

const ENCODER_MODEL_PATH:&'static str = "models/encoder_cpu.onnx";
const STYLE_ENCODER_MODEL_PATH:&'static str = "models/style_encoder_cpu.onnx";
const MODEL_INPUT_WIDTH:usize = 255;
const MODEL_INPUT_HEIGHT:usize = 255;
const MODEL_LATENT_SIZE:usize = 128;

//static ref ENCODER_MODEL:tch::CModule = tch::CModule::load(ENCODER_MODEL_PATH).expect("Failed to find models at expected location: models/traced_*coder_cpu.pt");
lazy_static! {
	//static ref MODEL:SimplePlan<TypedFact, Box<dyn TypedOp>, tract_onnx::prelude::Graph<TypedFact, Box<dyn TypedOp>>> =
}

pub fn mlhash(img:&DynamicImage) -> Vec<u8> {
	//hash(img, &MODEL)
	todo!()
}
