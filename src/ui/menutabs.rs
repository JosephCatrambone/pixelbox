use std::path::Path;
use crate::Engine;
use crate::AppTab;
use crate::MainApp;
use eframe::egui;

pub fn navigation(app_state: &mut MainApp, ui: &mut egui::Ui) {
	// Main Menu
	egui::menu::bar(ui, |ui| {
		ui.menu_button("File", |ui| {
			if ui.button("New DB").clicked() {
				if let Some(file_path) = rfd::FileDialog::new().add_filter("SQLite DB", &["db", "sqlite"]).save_file() {
					// TODO: Shutdown old engine.
					app_state.image_id_to_texture_handle.clear();
					app_state.engine = Some(Engine::new(Path::new(&file_path)));
					app_state.active_tab = AppTab::Folders;  // Transition right away to tracking new folders.
				}
				ui.close_menu();
			}
			if ui.button("Open DB").clicked() {
				if let Some(file_path) = rfd::FileDialog::new().add_filter("SQLite DB", &["db", "sqlite"]).pick_file() {
					app_state.engine = Some(Engine::open(Path::new(&file_path)));
					app_state.active_tab = AppTab::Search;
					app_state.image_id_to_texture_handle.clear();
				}
				ui.close_menu();
			}
			//if ui.button("Quit").clicked() { frame.quit(); }
		});

		ui.selectable_value(&mut app_state.active_tab, AppTab::Search, "Search");
		ui.selectable_value(&mut app_state.active_tab, AppTab::View, "View");
		ui.selectable_value(&mut app_state.active_tab, AppTab::Folders, "Folders");
		ui.selectable_value(&mut app_state.active_tab, AppTab::Settings, "Settings");
	});
}