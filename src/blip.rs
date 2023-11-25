// blip.rs is a standalone file because it's used both in the generation of tags and in the generation of image hashes.

#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;


use candle_core::{DType, Device, Result, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::blip;
use candle_transformers::models::quantized_blip;
use tokenizers::Tokenizer;
use image::DynamicImage;
use lazy_static::lazy_static;
use std::str::FromStr;
use image::imageops::FilterType;

lazy_static! {
	static ref DEVICE: Device = { Device::Cpu };
	static ref TOKENIZER: Tokenizer = { Tokenizer::from_str(include_str!("../models/blip-tokenizer.json")).unwrap() };
	static ref MODEL: quantized_blip::BlipForConditionalGeneration = {
		/*
		let api = hf_hub::api::sync::Api::new()?;
            if args.quantized {
                let api = api.model("lmz/candle-blip".to_string());
                api.get("blip-image-captioning-large-q4k.gguf")?
            } else {
                let api = api.repo(hf_hub::Repo::with_revision(
                    "Salesforce/blip-image-captioning-large".to_string(),
                    hf_hub::RepoType::Model,
                    "refs/pr/18".to_string(),
                ));
                api.get("model.safetensors")?
            }
		*/
		let config = blip::Config::image_captioning_large();
		let vb = quantized_blip::VarBuilder::from_gguf(MODEL_FILENAME).expect(&format!("Couldn't read GGUF at {MODEL_FILENAME}"));
		//let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[model_file], DType::F32, &device)? };
		let model: quantized_blip::BlipForConditionalGeneration = quantized_blip::BlipForConditionalGeneration::new(&config, vb).expect("Couldn't load model from file and configuration.");
		model
	};
}


const MODEL_FILENAME: &str = "./models/blip-image-captioning-large-q4k.gguf";
const SEP_TOKEN_ID: u32 = 102;

/// Loads an image from disk using the image crate, this returns a tensor with shape
/// (3, 384, 384). OpenAI normalization is applied.
//pub fn load_image<P: AsRef<std::path::Path>>(p: P) -> Result<Tensor> {
fn image_to_tensor(img: &DynamicImage) -> Result<Tensor> {
	let img = img.resize_to_fill(384, 384, FilterType::Triangle).to_rgb8();
	let data = img.into_raw();
	let data = Tensor::from_vec(data, (384, 384, 3), &Device::Cpu)?.permute((2, 0, 1))?;
	let mean = Tensor::new(&[0.48145466f32, 0.4578275, 0.40821073], &Device::Cpu)?.reshape((3, 1, 1))?;
	let std = Tensor::new(&[0.26862954f32, 0.261_302_6, 0.275_777_1], &Device::Cpu)?
		.reshape((3, 1, 1))?;
	(data.to_dtype(DType::F32)? / 255.)?
		.broadcast_sub(&mean)?
		.broadcast_div(&std)
}

fn image_to_internal_embedding(img: &DynamicImage) -> Tensor {
	let image_tensor = image_to_tensor(img).expect("Couldn't load dynamic image and convert to tensor.");
	image_tensor.unsqueeze(0).unwrap().apply(MODEL.vision_model()).unwrap()
}

fn internal_embedding_to_u8(image_embeds: &Tensor) -> Vec<u8> {
	let float_embed = image_embeds.flatten_all().unwrap().to_vec1::<f32>().unwrap();
	float_embed.iter().map(|f| { 128u8.saturating_add_signed((f*51.0f32).max(-128.0f32).min(-128.0f32) as i8) }).collect::<Vec<u8>>()
}

fn caption_image_internal(image_embeds: Tensor, max_tokens: Option<u32>) -> Result<String> {
	let mut logits_processor = candle_transformers::generation::LogitsProcessor::new(1337, None, None);
	let mut token_ids = vec![30522u32];
	for index in 0..max_tokens.unwrap_or(1000) {
		let context_size = if index > 0 { 1 } else { token_ids.len() };
		let start_pos = token_ids.len().saturating_sub(context_size);
		let input_ids = Tensor::new(&token_ids[start_pos..], &DEVICE)?.unsqueeze(0)?;
		let logits = MODEL.clone().text_decoder().forward(&input_ids, &image_embeds)?;
		let logits = logits.squeeze(0)?;
		let logits = logits.get(logits.dim(0)? - 1)?;
		let token = logits_processor.sample(&logits)?;
		if token == SEP_TOKEN_ID {
			break;
		}
		token_ids.push(token);
	}
	Ok(TOKENIZER.decode(&token_ids, true).unwrap_or("".to_string()))
}

pub fn caption_image(img: &DynamicImage, max_tokens: Option<u32>) -> String {
	let image_embeds = image_to_internal_embedding(img);
	caption_image_internal(image_embeds, max_tokens).unwrap()
}

pub fn embed_image(img: &DynamicImage) -> Vec<u8> {
	let image_embeds = image_to_internal_embedding(img);
	internal_embedding_to_u8(&image_embeds)
}

pub fn generate_embedding_and_caption(img: &DynamicImage, max_tokens: Option<u32>) -> (String, Vec<u8>) {
	let image_embeds = image_to_internal_embedding(img);
	let embeds = internal_embedding_to_u8(&image_embeds);
	let caption = caption_image_internal(image_embeds, max_tokens).unwrap();
	(caption, embeds)
}

#[cfg(test)]
mod test {
	use criterion;
	use image;
	use std::time::Instant;
	use super::*;
	//use crate::engine::hamming_distance;

	#[test]
	fn test_blip_hashes() {
		for filename in &[
			"test_resources/flat_white.png",
			"test_resources/phash_test_a.png",
			"test_resources/phash_test_crop.png",
			"test_resources/phash_test_not_a.png",
			"test_resources/phash_test_resize.png",
			"test_resources/phash_test_rot1.png",
		] {
			let img = image::open(filename).unwrap();
			println!("{}", &filename);
			let start = Instant::now();
			let hash = embed_image(&img);
			let end = Instant::now();
			println!("Took {:?} seconds", end-start);
		}
		//assert_eq!(hash, vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
	}
}