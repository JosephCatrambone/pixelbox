use crate::engine::Engine;
use crate::ui::paginate;
use eframe::{egui, epi, NativeOptions};
use rfd;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn folder_panel(
		engine: &mut Engine,
		ctx: &egui::Context,
		ui: &mut egui::Ui
) {
	let mut new_tracked_folder: Option<PathBuf> = None;
	let mut to_remove:Option<String> = None;
	
	//ui.heading("Watched Directories");
	//ui.collapsing("Watched Directories", |ui| {
	let scroll_area = egui::ScrollArea::vertical();
	scroll_area.max_height(ui.available_rect_before_wrap().height()).show(ui, |ui| {
		let folders = engine.get_tracked_folders();
		
		// New folder to add...
		if ui.button("Add Directory").clicked() {
			new_tracked_folder = rfd::FileDialog::new().pick_folder();
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

	// If we happen to be reindexing, show the most recent items and the progress so far.
	if engine.is_indexing_active() {
		egui::TopBottomPanel::bottom("bottom_panel")
			.resizable(true)
			.min_height(0.0)
			.show(ctx, |ui| {
				ui.label("Reindexing.");
				ui.vertical_centered(|ui| {
					for file in engine.get_last_indexed() {
						ui.label(file);
					}
				});
			});
	} else if let Some(new_folder) = new_tracked_folder {
		engine.add_tracked_folder(fs::canonicalize(new_folder).into());
	} else if let Some(dir_to_remove) = to_remove {
		engine.remove_tracked_folder(dir_to_remove);
	} else {
		egui::TopBottomPanel::bottom("bottom_panel")
			.resizable(true)
			.min_height(0.0)
			.show(ctx, |ui| {
				if ui.button("Reindex").clicked() {
					engine.start_reindexing();
				}
			});
	}
}