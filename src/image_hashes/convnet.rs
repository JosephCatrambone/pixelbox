
use image::{DynamicImage, GenericImageView};
use tch::{CModule, Tensor};
use std::ops::{AddAssign, DivAssign};

const ENCODER_MODEL_PATH:&'static str = "encoder.pt";
const MODEL_INPUT_WIDTH:u32 = 256;
const MODEL_INPUT_HEIGHT:u32 = 256;
const MODEL_LATENT_SIZE:u32 = 128;

lazy_static! {
	static ref encoder_model:tch::CModule = tch::CModule::load(ENCODER_MODEL_PATH).expect("Failed to find models at expected location: models/traced_*coder_cpu.pt");
}

pub fn mlhash(img:&DynamicImage) -> Vec<u8> {
	let t = image_to_tensor(&img);
	let latent = encoder.forward(&t.unsqueeze(0)).contiguous();
	//let mut repr = vec![0f64; LATENT_SIZE as usize];
	//unsafe { std::ptr::copy(latent.data_ptr(), repr.as_mut_ptr().cast(), LATENT_SIZE as usize) };
	let data: &[f32] = unsafe { std::slice::from_raw_parts(latent.data_ptr() as *const f32, LATENT_SIZE) };
	data.iter().map(|v|{ (128f32 + v.max(-1f32).min(1f32) * 128f32) as u8 }).to_vec()
}

fn image_to_tensor(img: &DynamicImage) -> Tensor {
	let img_resized = img.resize_to_fill(MODEL_INPUT_WIDTH, MODEL_INPUT_HEIGHT, image::imageops::Nearest);

	let u8tensor = Tensor::of_data_size(
		img_resized.as_bytes(),
		&[img_resized.width() as i64, img_resized.height() as i64, 3i64],
		tch::kind::Kind::Int8
	).permute(&[2, 0, 1]); // Convert from WHC to CHW.

	let mut t = Tensor::zeros(&[3, MODEL_INPUT_HEIGHT as i64, MODEL_INPUT_WIDTH as i64], tch::kind::FLOAT_CPU);
	t.add_assign(u8tensor);
	t.div_assign(255.0f32);

	t
}