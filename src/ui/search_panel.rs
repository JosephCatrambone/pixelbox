use std::borrow::BorrowMut;
use crate::engine::Engine;
use eframe::egui::{Ui, self};
use std::path::Path;


pub fn search_panel(engine: &mut Engine, ui: &mut Ui, search_text: &mut String) {
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
		// Default: show placeholder with 'drag image here or start typing'.
		// 'advanced' will drop down the option with checkboxes for 'search by style' and 'search by tags' or 'search by contents' or 'search by style'.

		// Indexing is not running, so show our search options:
		if ui.button("Search by Image").clicked() {
			let result = nfd::open_file_dialog(None, None).unwrap();
			match result {
				nfd::Response::Okay(file_path) => {
					*search_text = format!("file:{file_path}");
				},
				nfd::Response::OkayMultiple(files) => (),
				nfd::Response::Cancel => (),
			}
		}

		ui.separator();

		if ui.text_edit_singleline(search_text).changed() {
			engine.query_by_image_name(&search_text.clone());
		}

		ui.separator();

		//if ui.checkbox(advanced_mode, "Advanced Search").clicked() {}
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