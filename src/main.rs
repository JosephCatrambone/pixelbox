mod crawler;
mod engine;
mod image_hashes;
mod indexed_image;
mod ui;

use crate::indexed_image::{IndexedImage, THUMBNAIL_SIZE};
use eframe::{egui, self, NativeOptions};
use engine::Engine;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum AppTab {
	Start,
	Search,
	View,
	Folders,
	Settings,
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct MainApp {
	engine: Option<Engine>,
	active_tab: AppTab,
	image_id_to_texture_handle: HashMap::<i64, egui::TextureHandle>,  // For storing the thumbnails loaded.

	// Start Tab:
	
	// Search Tab:
	thumbnail_size: u8,
	search_text_min_length: u8,
	search_text: String,
	query_error: String,
	some_value: f32,
	current_page: u64,

	// View Tab:
	selected_image: Option<IndexedImage>, // Should we move this into the enum?
	full_image_path: String,
	full_image: Option<egui::TextureHandle>,
	zoom_level: f32,

	// Explore Tab:

	// Settings Tab:
	dark_mode: bool,

}

impl Default for MainApp {
	fn default() -> Self {
		MainApp {
			engine: None,
			active_tab: AppTab::Start,
			image_id_to_texture_handle: HashMap::new(),

			thumbnail_size: 128,
			search_text_min_length: 2,
			search_text: "".to_string(),
			query_error: "".to_string(),
			some_value: 1.0f32,
			current_page: 0u64,

			selected_image: None,
			full_image_path: "".to_string(),
			full_image: None,
			zoom_level: 1.0f32,

			dark_mode: true,
		}
	}
}

impl eframe::App for MainApp {
	fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
		// Enforce dark mode.
		ctx.set_visuals(if self.dark_mode { egui::Visuals::dark() } else { egui::Visuals::light() });

		// Display UI tabs:
		egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
			ui::menutabs::navigation(self, ui);
		});

		egui::CentralPanel::default().show(ctx, |ui| {
			match (&mut self.engine, &self.active_tab) {
				// If the engine is unloaded or we somehow get back to the start tab...
				(None, _) => ui::start::start_panel(ui),
				(_, AppTab::Start) => ui::start::start_panel(ui),
				// If the engine is loaded...
				(Some(_), AppTab::Search) => ui::search::search_panel(self, ui),
				(Some(engine), AppTab::Folders) => ui::folders::folder_panel(engine, ctx, ui),
				(Some(_), AppTab::View) => ui::view::view_panel(self, ui),
				(Some(_), AppTab::Settings) => ui::settings::settings_panel(self, ui),
				(Some(_), _) => ()
			}
		});
	}
}


fn main() {
	let app = MainApp::default();
	let options = eframe::NativeOptions {
		..Default::default()
	};
	// This is a bit hacky.  We could probably get away with just
	//eframe::run_native("PixelBox", options, Box::new(app);
	eframe::run_native("PixelBox", options, Box::new(|ctx| {
		egui_extras::install_image_loaders(&ctx.egui_ctx);
		Ok(Box::<MainApp>::new(app))
	}));
}

