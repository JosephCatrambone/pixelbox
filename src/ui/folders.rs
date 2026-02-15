use crate::engine::Engine;
use crate::ui::paginate;
use eframe::{egui, NativeOptions};
use rfd;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use crate::crawler::Crawler;

pub fn folder_panel(
		engine: &mut Engine,
		ctx: &egui::Context,
		ui: &mut egui::Ui
) {
	// We are kinda' assuming that these things can't happen in the same frame.
	let mut new_tracked_folder: Option<String> = None;
	let mut to_remove:Option<String> = None;
	
	//ui.heading("Watched Directories");
	//ui.collapsing("Watched Directories", |ui| {
	let scroll_area = egui::ScrollArea::vertical();
	scroll_area.max_height(ui.available_rect_before_wrap().height()).show(ui, |ui| {
		let folders = engine.get_tracked_folders();
		
		// New folder to add...
		if ui.button("Add Directory").clicked() {
			if let Some(new_path) = rfd::FileDialog::new().pick_folder() {
				//fs::canonicalize() should get us from a relative path to this, but I've not had problems so far without it and it adds a funky \\X?\ to Windows paths.
				new_tracked_folder = Some(new_path.as_path().to_str().unwrap().parse().unwrap());
			}
		}
		
		// Old folder to remove.
		for dir in folders {
			ui.horizontal(|ui|{
				ui.label(dir);
				if ui.button("x").clicked() {
					to_remove = Some(dir.clone());
				}
			});
		}
	});

	// Status Areas:
	egui::TopBottomPanel::bottom("bottom_panel")
		.resizable(true)
		.min_height(0.0)
		.show(ctx, |ui| {
			// Show Reindexing Button
			if engine.is_indexing_active() {
				ui.label(format!("Reindexing.  {} images indexed", engine.get_num_indexed_images()));
				if ui.button("Stop Indexing").clicked() {
					engine.stop_indexing();
				}
				//ui.vertical_centered(|ui| {});
				for file in engine.get_last_added() {
					ui.label(file);
				}
			} else {
				if ui.button("Reindex").clicked() {
					engine.start_indexing();
				}
			}
		});

	if !engine.is_indexing_active() {
		if let Some(new_folder) = new_tracked_folder {
			// New Folder Addition
			engine.add_tracked_folder(new_folder);
		} else if let Some(dir_to_remove) = to_remove {
			// Folder Removal
			engine.remove_tracked_folder(dir_to_remove);
		}
	}
}