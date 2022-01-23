
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

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
struct MainApp {
	engine: Option<Engine>,
	filename_search_text: String,
	some_value: f32,
	current_page: u64,

	image_id_to_texture_id: HashMap::<i64, egui::TextureId>
}

impl Default for MainApp {
	fn default() -> Self {
		MainApp {
			engine: None,
			filename_search_text: "".to_string(),
			some_value: 1.0f32,
			current_page: 0u64,
			image_id_to_texture_id: HashMap::new(),
		}
	}
}

impl epi::App for MainApp {
	fn update(&mut self, ctx: &egui::CtxRef, frame: &epi::Frame) {
		let MainApp {
			engine,
			filename_search_text,
			some_value,
			current_page,
			image_id_to_texture_id,
		} = self;

		egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
			// The top panel is often a good place for a menu bar:
			egui::menu::bar(ui, |ui| {
				ui.menu_button("File", |ui| {
					if ui.button("New DB").clicked() {
						let result = nfd::open_save_dialog(Some("db"), None).unwrap();
						match result {
							nfd::Response::Okay(file_path) => {
								// TODO: Shutdown old engine.
								*engine = Some(Engine::new(Path::new(&file_path)))
							},
							nfd::Response::OkayMultiple(files) => (),
							nfd::Response::Cancel => (),
						}
					}
					if ui.button("Open DB").clicked() {
						let result = nfd::open_file_dialog(Some("db"), None).unwrap();
						match result {
							nfd::Response::Okay(file_path) => *engine = Some(Engine::open(Path::new(&file_path))),
							nfd::Response::OkayMultiple(files) => (),
							nfd::Response::Cancel => (),
						}
					}
					if ui.button("Quit").clicked() {
						frame.quit();
					}
				});
			})
		});

		if let Some(engine) = engine {
			egui::SidePanel::left("side_panel").show(ctx, |ui| {
				ui::search_panel(engine, ui);
			});
		}

		egui::CentralPanel::default().show(ctx, |ui| {
			ui.heading("Search Results for Image");
			//ui.hyperlink("https://github.com/emilk/egui_template");
			//ui.add(egui::github_link_file_line!("https://github.com/emilk/egui_template/blob/master/", "Direct link to source code."));
			//egui::warn_if_debug_build(ui);
			ui.separator();

			//ui.label("The central panel the region left after adding TopPanel's and SidePanel's");
			if let Some(engine) = engine {
				if let Some(results) = engine.get_query_results() {
					//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));
					let num_results = results.len();
					let page_size = 10;

					let scroll_area = egui::ScrollArea::vertical();
					scroll_area.max_height(ui.available_rect_before_wrap().height()).show(ui, |ui| {
						ui::image_table(ui, ctx, frame, results, image_id_to_texture_id, (THUMBNAIL_SIZE.0 as f32, THUMBNAIL_SIZE.1 as f32));
					});

					// Pagination:
					ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
						ui::paginate(ui, current_page, (num_results/page_size) as u64);
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

