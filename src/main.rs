
mod engine;
mod image_hashes;
mod indexed_image;
mod ui;

use eframe::{egui, epi};
use engine::Engine;
use nfd;
use std::path::Path;
use std::time::Duration;

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
struct MainApp {
	engine: Option<Engine>,
	some_text: String,
	some_value: f32,
}

impl Default for MainApp {
	fn default() -> Self {
		MainApp {
			engine: None,
			some_text: "LMAO".to_string(),
			some_value: 1.0f32,
		}
	}
}

impl epi::App for MainApp {
	fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
		let MainApp {
			engine,
			some_text,
			some_value,
		} = self;

		egui::TopPanel::top("top_panel").show(ctx, |ui| {
			// The top panel is often a good place for a menu bar:
			egui::menu::bar(ui, |ui| {
				egui::menu::menu(ui, "File", |ui| {
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
			});
		});

		if let Some(engine) = engine {
			egui::SidePanel::left("side_panel", 200.0).show(ctx, |ui| {

				// Special button creation for reindex.
				if ui.add(egui::Button::new("Reindex").enabled(!engine.is_indexing_active())).clicked() {
					engine.start_reindexing();
				}

				//ui.heading("Watched Directories");
				ui.collapsing("Watched Directories", |ui| {
					let folders = engine.get_tracked_folders();
					let mut to_remove:Option<String> = None;
					for dir in folders {
						ui.horizontal(|ui|{
							ui.label(dir);
							if ui.button("x").clicked() {
								to_remove = Some(dir.clone());
							}
						});
					}
					if ui.button("Add Directory").clicked() {
						let result = nfd::open_pick_folder(None).unwrap();
						match result {
							nfd::Response::Okay(file_path) => engine.add_tracked_folder(file_path),
							_ => ()
						}
					}
					if let Some(dir_to_remove) = to_remove {
						engine.remove_tracked_folder(dir_to_remove);
					}
				});

				ui.horizontal(|ui| {
					ui.label("Write something: ");
					ui.text_edit_singleline(some_text);
				});

				ui.add(egui::Slider::f32(some_value, 0.0..=10.0).text("value"));
				if ui.button("Increment").clicked() {
					*some_value += 1.0;
				}

				ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
					ui.add(
						egui::Hyperlink::new("https://github.com/emilk/egui/").text("powered by egui"),
					);
				});
			});
		}

		egui::CentralPanel::default().show(ctx, |ui| {
			ui.heading("egui template");
			ui.hyperlink("https://github.com/emilk/egui_template");
			ui.add(egui::github_link_file_line!(
                "https://github.com/emilk/egui_template/blob/master/",
                "Direct link to source code."
            ));
			egui::warn_if_debug_build(ui);

			ui.separator();

			ui.heading("Central Panel");
			ui.label("The central panel the region left after adding TopPanel's and SidePanel's");
			ui.label("It is often a great place for big things, like drawings:");

			/*
			ui.heading("Draw with your mouse to paint:");
			painting.ui_control(ui);
			egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
				painting.ui_content(ui);
			});
			 */
		});

		if false {
			egui::Window::new("Window").show(ctx, |ui| {
				ui.label("Windows can be moved by dragging them.");
				ui.label("They are automatically sized based on contents.");
				ui.label("You can turn on resizing and scrolling if you like.");
				ui.label("You would normally chose either panels OR windows.");
			});
		}
	}

	fn name(&self) -> &str {
		"PixelBox Image Search"
	}

	fn is_resizable(&self) -> bool {
		true
	}
}


fn main() {
	let app = MainApp::default();
	eframe::run_native(Box::new(app));
}