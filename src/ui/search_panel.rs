use std::borrow::BorrowMut;
use crate::engine::Engine;
use eframe::egui::{Ui, self};
use std::path::Path;


pub fn search_panel(self: &mut MainApp, engine: &mut Engine, ui: &mut Ui, search_text: &mut String) {
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


pub fn other_search_panel(engine: &mut Engine, ui: &mut Ui, search_text: &mut String) {
	/*
		ui.heading("Draw with your mouse to paint:");
		painting.ui_control(ui);
		egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
			painting.ui_content(ui);
		});
	*/

	// Special button creation for reindex.
	ui.group(|ui|{
		ui.set_enabled(!engine.is_indexing_active());
		if ui.button("Reindex").clicked() {
			engine.start_reindexing();
		}
	});

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

	// If indexing is running, show the status..
	if engine.is_indexing_active() {
		ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
			ui.vertical(|ui|{
				ui.label("Indexed:");
				for entry in engine.get_last_indexed() {
					ui.label(entry);
				}
			});
		});
	} else {
		// Indexing is not running, so show our search options:
		if ui.button("Search by Image").clicked() {
			let result = nfd::open_file_dialog(None, None).unwrap();
			match result {
				nfd::Response::Okay(file_path) => engine.query_by_image_hash_from_file(Path::new(&file_path)),
				nfd::Response::OkayMultiple(files) => (),
				nfd::Response::Cancel => (),
			}
		}

		ui.separator();

		if ui.text_edit_singleline(search_text).changed() || ui.button("Search by Text").clicked() {
			engine.query_by_image_name(&search_text.clone());
		}
	}

	/*
	ui.horizontal(|ui| {
		//ui.label("Search by Filename: ");
		ui.text_edit_singleline(filename_search_text);
		if ui.button("Search by Filename").clicked() {
			engine.query_by_image_name(filename_search_text.clone());
		}
	});

	ui.add(egui::Slider::f32(some_value, 0.0..=10.0).text("Min Sim"));
	if ui.button("Increment").clicked() {
		*some_value += 1.0;
	}
	*/
}