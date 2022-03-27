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
				let result = nfd::open_save_dialog(Some("db"), None).unwrap();
				match result {
					nfd::Response::Okay(file_path) => {
						// TODO: Shutdown old engine.
						app_state.engine = Some(Engine::new(Path::new(&file_path)));
						app_state.active_tab = AppTab::Folders;  // Transition right away to tracking new folders.
					},
					nfd::Response::OkayMultiple(files) => (),
					nfd::Response::Cancel => (),
				}
				ui.close_menu();
			}
			if ui.button("Open DB").clicked() {
				let result = nfd::open_file_dialog(Some("db"), None).unwrap();
				match result {
					nfd::Response::Okay(file_path) => app_state.engine = Some(Engine::open(Path::new(&file_path))),
					nfd::Response::OkayMultiple(files) => (),
					nfd::Response::Cancel => (),
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