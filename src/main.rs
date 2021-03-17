
mod crawler;
mod engine;
mod image_hashes;
mod indexed_image;
mod ui;

use crate::indexed_image::{IndexedImage, THUMBNAIL_SIZE};
use eframe::{egui, epi};
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
	fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
		let MainApp {
			engine,
			filename_search_text,
			some_value,
			current_page,
			image_id_to_texture_id,
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

				if ui.button("Search by Image").clicked() {
					let result = nfd::open_file_dialog(None, None).unwrap();
					match result {
						nfd::Response::Okay(file_path) => engine.query_by_image_hash_from_file(Path::new(&file_path)),
						nfd::Response::OkayMultiple(files) => (),
						nfd::Response::Cancel => (),
					}
				}

				ui.horizontal(|ui| {
					//ui.label("Search by Filename: ");
					ui.text_edit_singleline(filename_search_text);
					if ui.button("Search by Filename").clicked() {
						engine.query_by_image_name(filename_search_text.clone());
					}
				});

				ui.add(egui::Slider::f32(some_value, 0.0..=10.0).text("value"));
				if ui.button("Increment").clicked() {
					*some_value += 1.0;
				}
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
					let num_columns = (ui.available_width() / THUMBNAIL_SIZE.0 as f32).max(1.0f32) as usize;
					//let num_rows = num_results / num_columns;

					let scroll_area = egui::ScrollArea::from_max_height(ui.available_rect_before_wrap().height());
					scroll_area.show(ui, |ui| {
						egui::Grid::new("image_result_grid")
							.striped(false)
							.min_col_width(THUMBNAIL_SIZE.0 as f32)
							.max_col_width(THUMBNAIL_SIZE.0 as f32)
							.show(ui, |ui| {
								for row in 0..(num_results / num_columns) {
									for col in 0..num_columns {
										let res = &results[col + row * num_columns];
										let tex_id = match image_id_to_texture_id.get(&res.id) {
											Some(tid) => *tid,
											None => {
												let tid = thumbnail_to_egui_element(&results[col + row * num_columns], frame);
												image_id_to_texture_id.insert(results[col + row * num_columns].id, tid);
												tid
											}
										};
										//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));
										ui.image(tex_id, [res.thumbnail_resolution.0 as f32, res.thumbnail_resolution.1 as f32]);
										//ui.label(format!("Img: {}", &results[col + row*num_columns].filename));
										// To handle right click:
										//ui.button("Test").secondary_clicked()
									}
									ui.end_row();
								}
							});
					});

					// Pagination:
					ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
						ui.horizontal(|ui|{
							if ui.button("<<").clicked() {
								*current_page = 0;
							}
							if ui.button("<").clicked() {
								if *current_page > 1 {
									*current_page -= 1;
								}
							}
							ui.label(format!("Page {} of {}", *current_page, 10));
							if ui.button(">").clicked() {
								if *current_page < 9 {
									*current_page += 1;
								}
							}
							if ui.button(">>").clicked() {
								*current_page = 10;
							}
						});
						//ui.add(egui::Hyperlink::new("https://github.com/emilk/egui/").text("powered by egui"),);
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

fn thumbnail_to_egui_element(img:&indexed_image::IndexedImage, frame: &mut epi::Frame<'_>) -> egui::TextureId {
	let tex_allocator = frame.tex_allocator();
	let mut pixels = Vec::<egui::Color32>::with_capacity(img.thumbnail.len()/3);
	for i in (0..img.thumbnail.len()).step_by(3) {
		let r = img.thumbnail[i];
		let g = img.thumbnail[i+1];
		let b = img.thumbnail[i+2];
		pixels.push(egui::Color32::from_rgb(r, g, b));
	}
	let texture_id = tex_allocator.alloc_srgba_premultiplied((img.thumbnail_resolution.0 as usize, img.thumbnail_resolution.1 as usize), pixels.as_slice());
	texture_id
}

fn free_thumbnail(img:egui::TextureId, frame: &mut epi::Frame<'_>) {
	let allocator = frame.tex_allocator();
	allocator.free(img);
}