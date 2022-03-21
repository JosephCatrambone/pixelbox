
use eframe::{egui, epi, NativeOptions};
use crate::engine::Engine;

pub fn start_panel(
		ctx: &egui::CtxRef,
		frame: &epi::Frame,
		ui: &mut egui::Ui
) {
	/*
	ui.horizontal(|ui|{
		if ui.button("Search by Image").clicked() {
		}
	});
	*/

	ui.vertical(|ui|{
		ui.heading("Welcome to PixelBox");
		ui.label("To Begin:");
		ui.label(" 1. Create a New Image Database (File > New DB)");
		ui.label(" 2. Add Tracked Folders (Folders > Add Directory)");
		ui.label(" 3. Reindex!");
		//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));
	
		ui.hyperlink("https://github.com/josephcatrambone/pixelbox");
		//ui.add(egui::github_link_file_line!("https://github.com/josephcatrambone/pixelbox", "Written by Joseph Catrambone for Xoana LTD - Offered under MIT License"));
		//ui.separator();
	});
	
	//egui::warn_if_debug_build(ui);
}