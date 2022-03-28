
mod crawler;
mod engine;
mod image_hashes;
mod indexed_image;
mod ui;

use crate::indexed_image::{IndexedImage, THUMBNAIL_SIZE};
use eframe::{egui, epi, NativeOptions};
use engine::Engine;
use nfd;
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
	search_text: String,
	some_value: f32,
	current_page: u64,

	// View Tab:
	selected_image: Option<IndexedImage>, // Should we move this into the enum?

	// Explore Tab:

	// Settings Tab:


}

impl Default for MainApp {
	fn default() -> Self {
		MainApp {
			engine: None,
			active_tab: AppTab::Start,
			image_id_to_texture_handle: HashMap::new(),
			
			search_text: "".to_string(),
			some_value: 1.0f32,
			current_page: 0u64,

			selected_image: None,
		}
	}
}

impl epi::App for MainApp {
	fn update(&mut self, ctx: &egui::Context, frame: &epi::Frame) {
		// Enforce dark mode.
		ctx.set_visuals(egui::Visuals::dark());

		// Display UI tabs:
		egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
			ui::menutabs::navigation(self, ui);
		});

		egui::CentralPanel::default().show(ctx, |ui|{
			if let Some(engine) = &mut self.engine {
				match self.active_tab {
					AppTab::Start => {
					
					},
					AppTab::Search => {
						ui::search::search_panel(self, ui);
					},
					AppTab::Folders => {
						ui::folders::folder_panel(engine, ctx, ui);
					},
					_ => (),
				}
			} else {
				ui::start::start_panel(ui);
			}
		});
	}

	fn name(&self) -> &str {
		"PixelBox Image Search"
	}
}


fn main() {
	let app = MainApp::default();
	let options = eframe::NativeOptions {
		drag_and_drop_support: true,
		..Default::default()
	};
	eframe::run_native(Box::new(app), options);
}

