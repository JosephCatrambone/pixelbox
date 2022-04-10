pub mod menutabs;
pub mod search;
pub mod start;
pub mod folders;
pub mod view;

use std::collections::HashMap;
use eframe::egui;
use eframe::egui::ColorImage;
use eframe::egui::Ui;
use egui_extras::RetainedImage;
use image;
use tract_onnx::prelude::tract_itertools::Itertools;

use crate::indexed_image;
use crate::indexed_image::IndexedImage;

fn load_image_from_path(path: &std::path::Path) -> Result<ColorImage, image::ImageError> {
	let image = image::io::Reader::open(path)?.decode()?;
	let size = [image.width() as _, image.height() as _];
	let image_buffer = image.to_rgba8();
	let pixels = image_buffer.as_flat_samples();
	Ok(egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice(),))
}

fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
	let image = image::load_from_memory(image_data)?;
	let size = [image.width() as _, image.height() as _];
	let image_buffer = image.to_rgba8();
	let pixels = image_buffer.as_flat_samples();
	Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice(),))
}

fn indexed_image_to_egui_colorimage(indexed_image: &IndexedImage, alpha_fill:u8) -> ColorImage {
	let num_pixels = indexed_image.thumbnail_resolution.0 * indexed_image.thumbnail_resolution.1;
	let mut new_vec = Vec::with_capacity((num_pixels / 3 * 4) as usize);
	indexed_image.thumbnail.chunks(3).for_each(|p|{
		new_vec.extend(p);
		new_vec.push(alpha_fill);
	});
	ColorImage::from_rgba_unmultiplied(
		[indexed_image.thumbnail_resolution.0 as usize, indexed_image.thumbnail_resolution.1 as usize],
		new_vec.as_slice()
	)
}

/// Given the thumbnail cache and an image ID, will attempt to load the TextureID from the cache.
/// On a cache hit, will return the TextureID.
/// On a cache miss, will take the RGB enumeration and generate a new thumbnail, then return the ID.
pub fn fetch_or_generate_thumbnail(res: &IndexedImage, thumbnail_cache: &mut HashMap::<i64, egui::TextureHandle>, ctx: &egui::Context) -> egui::TextureHandle {
	match thumbnail_cache.get(&res.id) {
		Some(tid) => tid.clone(),
		None => {
			let texture = ctx.load_texture(res.path.clone(), indexed_image_to_egui_colorimage(res, 255u8));
			thumbnail_cache.insert(res.id, texture.clone());
			texture
		}
	}
}

pub fn paginate(ui: &mut Ui, current_page: &mut u64, max_page: u64) {
	ui.horizontal(|ui|{
		if ui.button("<<").clicked() {
			*current_page = 0;
		}
		if ui.button("<").clicked() {
			if *current_page > 1 {
				*current_page -= 1;
			}
		}
		ui.label(format!("Page {} of {}", *current_page, max_page));
		if ui.button(">").clicked() {
			if *current_page < max_page {
				*current_page += 1;
			}
		}
		if ui.button(">>").clicked() {
			*current_page = max_page;
		}
	});
	//ui.add(egui::Hyperlink::new("https://github.com/emilk/egui/").text("powered by egui"),);
}

/// Example code for painting on a canvas with your mouse
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
struct Painting {
	lines: Vec<Vec<egui::Pos2>>,
	stroke: egui::Stroke,
}

impl Default for Painting {
	fn default() -> Self {
		Self {
			lines: Default::default(),
			stroke: egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE),
		}
	}
}

impl Painting {
	pub fn ui_control(&mut self, ui: &mut egui::Ui) -> egui::Response {
		ui.horizontal(|ui| {
			egui::stroke_ui(ui, &mut self.stroke, "Stroke");
			ui.separator();
			if ui.button("Clear Painting").clicked() {
				self.lines.clear();
			}
		})
			.response
	}

	pub fn ui_content(&mut self, ui: &mut egui::Ui) -> egui::Response {
		use egui::emath::{Pos2, Rect, RectTransform};

		let (mut response, painter) =
			ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

		let to_screen = RectTransform::from_to(
			Rect::from_min_size(Pos2::ZERO, response.rect.square_proportions()),
			response.rect,
		);
		let from_screen = to_screen.inverse();

		if self.lines.is_empty() {
			self.lines.push(vec![]);
		}

		let current_line = self.lines.last_mut().unwrap();

		if let Some(pointer_pos) = response.interact_pointer_pos() {
			let canvas_pos = from_screen * pointer_pos;
			if current_line.last() != Some(&canvas_pos) {
				current_line.push(canvas_pos);
				response.mark_changed();
			}
		} else if !current_line.is_empty() {
			self.lines.push(vec![]);
			response.mark_changed();
		}

		let mut shapes = vec![];
		for line in &self.lines {
			if line.len() >= 2 {
				let points: Vec<Pos2> = line.iter().map(|p| to_screen * *p).collect();
				shapes.push(egui::Shape::line(points, self.stroke));
			}
		}
		painter.extend(shapes);

		response
	}
}