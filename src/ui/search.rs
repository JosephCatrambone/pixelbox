use crate::engine::Engine;
use crate::ui::paginate;
use crate::ui::image_table;
use eframe::{egui, epi, NativeOptions};
use nfd;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub fn search_panel(
		engine: &mut Engine,
		image_id_to_texture_id: &mut HashMap::<i64, egui::TextureId>,
		search_text: &mut String,
		current_page: &mut u64,
		ctx: &egui::CtxRef,
		frame: &epi::Frame,
		ui: &mut egui::Ui
) {
	ui.horizontal(|ui|{
		if ui.button("Search by Image").clicked() {
			let result = nfd::open_file_dialog(None, None).unwrap();
			match result {
				nfd::Response::Okay(file_path) => engine.query_by_image_hash_from_file(Path::new(&file_path)),
				_ => (),
			}
		}
		// Universal Search
		if ui.text_edit_singleline(search_text).clicked() {}
	});

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
					paginate(ui, current_page, (num_results / page_size) as u64);
					//});
				});
			});
	}

	egui::CentralPanel::default().show(ctx, |ui| {
		ui.heading("Search Results for Image");
		//ui.hyperlink("https://github.com/emilk/egui_template");
		//ui.add(egui::github_link_file_line!("https://github.com/emilk/egui_template/blob/master/", "Direct link to source code."));
		//egui::warn_if_debug_build(ui);
		ui.separator();

		//ui.label("The central panel the region left after adding TopPanel's and SidePanel's");
		if let Some(results) = engine.get_query_results() {
			//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));

			let scroll_area = egui::ScrollArea::vertical();
			scroll_area.max_height(ui.available_rect_before_wrap().height()).show(ui, |ui| {
				image_table(ui, ctx, frame, results, image_id_to_texture_id);
			});
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