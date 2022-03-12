
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
		ui.label("To Begin, Create a New Image Database and Add Tracked Folders");
		//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));
	
		ui.hyperlink("https://github.com/josephcatrambone/pixelbox");
		//ui.add(egui::github_link_file_line!("https://github.com/emilk/egui_template/blob/master/", "Direct link to source code."));
		//ui.separator();
	});
	
	//egui::warn_if_debug_build(ui);
}