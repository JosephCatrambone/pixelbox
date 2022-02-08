use std::borrow::BorrowMut;
use crate::engine::Engine;
use eframe::egui::{Ui, self};
use std::path::Path;
use tract_onnx::tract_core::downcast_rs::Downcast;


pub fn search_panel(engine: &mut Engine, ui: &mut Ui) {
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

		let mut searched_image_name = String::new();  // TODO: Replace this with a static mut reference so the search string is editable.

		ui.text_edit_singleline(&mut searched_image_name);
		let img_name = searched_image_name.clone();
		if ui.button("Search by Text").clicked() {
			engine.query_by_image_name(&img_name);
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