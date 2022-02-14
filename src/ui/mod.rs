mod image_grid;
mod image_table;
mod search_panel;
pub mod search;
pub mod folders;

use eframe::{egui::{self, Ui}, epi};

pub use image_grid::image_grid;
pub use image_table::image_table;
pub use search_panel::search_panel;

use crate::indexed_image;
use crate::indexed_image::IndexedImage;

fn thumbnail_to_egui_element(img:&indexed_image::IndexedImage, ctx: &egui::Context, frame: &epi::Frame) -> egui::TextureId {
	let mut pixels = Vec::<u8>::with_capacity(img.thumbnail.len() + img.thumbnail.len()/3);
	for i in (0..img.thumbnail.len()).step_by(3) {
		pixels.push(img.thumbnail[i]);
		pixels.push(img.thumbnail[i+1]);
		pixels.push(img.thumbnail[i+2]);
		pixels.push(255u8);
	}
	if (img.thumbnail_resolution.0*img.thumbnail_resolution.1*4) as u32 != pixels.len() as u32 {
		eprintln!("Resolution/byte mismatch.");
		dbg!("{:?} {:?}", img.thumbnail_resolution, pixels.len());
		eprintln!("Corrupt thumbnail: fixme and/or make a recovery op, like empty-fill.");  // TODO
	}
	//let texture_id = tex_allocator.alloc_srgba_premultiplied((img.thumbnail_resolution.0 as usize, img.thumbnail_resolution.1 as usize), pixels.as_slice());
	let tex = epi::Image::from_rgba_unmultiplied([img.thumbnail_resolution.0 as usize, img.thumbnail_resolution.1 as usize], &pixels);
	let texture_id = frame.alloc_texture(tex);
	texture_id
}

fn free_thumbnail(img:egui::TextureId, frame: &mut epi::Frame) {
	frame.free_texture(img);
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

pub fn on_right_click(ui: &mut Ui, ctx:&egui::CtxRef, img: &IndexedImage) {
	egui::Window::new(&img.filename).show(ctx, |ui| {
		ui.label("Windows can be moved by dragging them.");
		ui.label("They are automatically sized based on contents.");
		ui.label("You can turn on resizing and scrolling if you like.");
		ui.label("You would normally chose either panels OR windows.");
	});
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