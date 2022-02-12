
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
enum AppTab {
	Search,
	View,
	Folders,
	Settings,
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
struct MainApp {
	engine: Option<Engine>,
	active_tab: AppTab,
	image_id_to_texture_id: HashMap::<i64, egui::TextureId>,  // For storing the thumbnails loaded.

	// Search Tab:
	search_text: String,
	some_value: f32,
	current_page: u64,

	// View Tab:

	// Explore Tab:

	// Settings Tab:


}

impl Default for MainApp {
	fn default() -> Self {
		MainApp {
			engine: None,
			active_tab: AppTab::Search,
			image_id_to_texture_id: HashMap::new(),
			
			search_text: "".to_string(),
			some_value: 1.0f32,
			current_page: 0u64,
			
		}
	}
}

impl epi::App for MainApp {
	fn update(&mut self, ctx: &egui::CtxRef, frame: &epi::Frame) {
		egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
			// Main Menu
			egui::menu::bar(ui, |ui| {
				ui.menu_button("File", |ui| {
					if ui.button("New DB").clicked() {
						let result = nfd::open_save_dialog(Some("db"), None).unwrap();
						match result {
							nfd::Response::Okay(file_path) => {
								// TODO: Shutdown old engine.
								self.engine = Some(Engine::new(Path::new(&file_path)))
							},
							nfd::Response::OkayMultiple(files) => (),
							nfd::Response::Cancel => (),
						}
						ui.close_menu();
					}
					if ui.button("Open DB").clicked() {
						let result = nfd::open_file_dialog(Some("db"), None).unwrap();
						match result {
							nfd::Response::Okay(file_path) => self.engine = Some(Engine::open(Path::new(&file_path))),
							nfd::Response::OkayMultiple(files) => (),
							nfd::Response::Cancel => (),
						}
						ui.close_menu();
					}
					if ui.button("Quit").clicked() {
						frame.quit();
					}
				});
				
				ui.selectable_value(&mut self.active_tab, AppTab::Search, "Search");
				ui.selectable_value(&mut self.active_tab, AppTab::View, "View");
				ui.selectable_value(&mut self.active_tab, AppTab::Folders, "Folders");
				ui.selectable_value(&mut self.active_tab, AppTab::Settings, "Settings");
			});
		});

		egui::CentralPanel::default().show(ctx, |ui|{
			// Actual search menu.
			if let Some(engine) = &mut self.engine {
				ui::search::search_panel(
					engine,
					&mut self.image_id_to_texture_id,
					&mut self.search_text,
					&mut self.current_page,
					ctx,
					frame,
					ui
				);
			}
		});
	}

	fn name(&self) -> &str {
		"PixelBox Image Search"
	}
}


fn main() {
	let app = MainApp::default();
	eframe::run_native(Box::new(app), NativeOptions::default());
}

