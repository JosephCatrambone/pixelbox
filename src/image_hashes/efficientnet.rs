use image::{DynamicImage, GenericImageView, imageops::FilterType};
use lazy_static::lazy_static;
use tract_onnx::prelude::*;

const SIMILARITY_MODEL_PATH:&'static str = "models/image_similarity.onnx";
const MODEL_INPUT_WIDTH:u32 = 224;
const MODEL_INPUT_HEIGHT:u32 = 224;
const MODEL_LATENT_SIZE:usize = 8;

lazy_static! {
	static ref MODEL: RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>> = {
		tract_onnx::onnx().model_for_path(SIMILARITY_MODEL_PATH).expect("Unable to load similarity model from disk!").into_optimized().unwrap().into_runnable().unwrap()
	};
}

/// Loads an image from disk using the image crate, this returns a tensor with shape
/// (3, 384, 384). OpenAI normalization is applied.
//pub fn load_image<P: AsRef<std::path::Path>>(p: P) -> Result<Tensor> {
fn image_to_tensor(img: &DynamicImage) -> Tensor {
	let img = img.resize_to_fill(MODEL_INPUT_WIDTH, MODEL_INPUT_HEIGHT, FilterType::Triangle).to_rgb8();
	let data: Tensor = tract_ndarray::Array4::from_shape_fn((1, 3, 224, 224), |(_, c, y, x)| {
		let mean = [0.485, 0.456, 0.406][c];
		let std = [0.229, 0.224, 0.225][c];
		(img[(x as _, y as _)][c] as f32 / 255.0 - mean) / std
	}).into();
	data
}

pub fn mlhash(img:&DynamicImage) -> Vec<u8> {
	//let model = tract_onnx::onnx().model_for_path(SIMILARITY_MODEL_PATH).expect("Unable to load similarity model from disk!").into_optimized().unwrap().into_runnable().unwrap();
	let img_tensor = image_to_tensor(img);
	let output = MODEL.run(tvec!(img_tensor.into())).unwrap();
	let float_embed = output[0]
		.to_array_view::<f32>()
		.unwrap()
		.iter()
		.map(|f| { 128u8.saturating_add_signed((f*128.0f32).max(-128.0f32).min(-128.0f32) as i8) })
		.collect::<Vec<u8>>();
	float_embed
}
