
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

enum AppTab {
	Search,
	View,
	Explore,
	Settings,
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
struct MainApp {
	engine: Option<Engine>,
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
			search_text: "".to_string(),
			some_value: 1.0f32,
			current_page: 0u64,
			image_id_to_texture_id: HashMap::new(),
		}
	}
}

impl epi::App for MainApp {
	fn update(&mut self, ctx: &egui::CtxRef, frame: &epi::Frame) {
		/*
		let MainApp {
			engine,
			search_text,
			some_value,
			current_page,
			image_id_to_texture_id,
		} = self;
		*/

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
			});

			// Actual search menu.
			ui.horizontal(|ui|{
				if let Some(engine) = &mut self.engine {
					if ui.button("Search by Image").clicked() {
						let result = nfd::open_file_dialog(None, None).unwrap();
						match result {
							nfd::Response::Okay(file_path) => engine.query_by_image_hash_from_file(Path::new(&file_path)),
							_ => (),
						}
					}
					// Universal Search
					if ui.text_edit_singleline(&mut self.search_text).clicked() {}
				}
			});

		});

		if let Some(engine) = &self.engine {
			if let Some(results) = engine.get_query_results() {
				let num_results = results.len();
				let page_size = 20;

				egui::TopBottomPanel::bottom("bottom_panel")
					.resizable(false)
					.min_height(0.0)
					.show(ctx, |ui| {
						ui.vertical_centered(|ui| {
							// Pagination:
							//ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
							ui::paginate(ui, &mut self.current_page, (num_results / page_size) as u64);
							//});
						});
					});
			}
		}

		egui::CentralPanel::default().show(ctx, |ui| {
			ui.heading("Search Results for Image");
			//ui.hyperlink("https://github.com/emilk/egui_template");
			//ui.add(egui::github_link_file_line!("https://github.com/emilk/egui_template/blob/master/", "Direct link to source code."));
			//egui::warn_if_debug_build(ui);
			ui.separator();

			//ui.label("The central panel the region left after adding TopPanel's and SidePanel's");
			if let Some(engine) = &self.engine {
				if let Some(results) = engine.get_query_results() {
					//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));

					let scroll_area = egui::ScrollArea::vertical();
					scroll_area.max_height(ui.available_rect_before_wrap().height()).show(ui, |ui| {
						ui::image_table(ui, ctx, frame, results, &mut self.image_id_to_texture_id);
					});
				}
			}
			/*
			ui.heading("Draw with your mouse to paint:");
			painting.ui_control(ui);
			egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
				painting.ui_content(ui);
			});
			 */
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

