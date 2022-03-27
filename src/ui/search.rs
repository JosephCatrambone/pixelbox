use crate::MainApp;
use crate::engine::Engine;
use crate::ui::{fetch_or_generate_thumbnail, paginate};
use eframe::{egui, epi, NativeOptions};
use eframe::egui::{Context, TextureHandle, Ui};
use nfd;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub fn search_panel(
	app_state: &mut MainApp,
	ui: &mut egui::Ui
) {
	if app_state.engine.is_none() {
		ui.label("To search for an image, make sure a DB is loaded and folders have been indexed.");
		return;
	}

	ui.horizontal(|ui|{
		if ui.button("Search by Image").clicked() {
			let result = nfd::open_file_dialog(None, None).unwrap();
			match result {
				nfd::Response::Okay(file_path) => app_state.engine.as_mut().unwrap().query_by_image_hash_from_file(Path::new(&file_path)),
				_ => (),
			}
		}
		
		// Universal Search
		if ui.text_edit_singleline(&mut app_state.search_text).changed() {
			app_state.engine.as_mut().unwrap().query_by_image_name(&app_state.search_text.clone())
		}
	});

	if let Some(results) = app_state.engine.as_ref().unwrap().get_query_results() {
		ui.heading("Results");
		//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));

		egui::ScrollArea::vertical()
			.auto_shrink([false, false])
			.show(ui, |ui| {
				ui.vertical(|ui|{
					results.iter().for_each(|res|{
						ui.horizontal(|ui|{
							let tex_id = fetch_or_generate_thumbnail(res, &mut app_state.image_id_to_texture_handle, ui.ctx());

							ui.image(&tex_id, [res.thumbnail_resolution.0 as f32, res.thumbnail_resolution.1 as f32]).context_menu(|ui|{
								if ui.button("Open").clicked() {
									//let _ = std::process::Command::new("open").arg(&res.path).output();
									open::that(&res.path);
									ui.close_menu();
								}
								if ui.button("Search for Similar").clicked() {
									app_state.engine.as_mut().unwrap().query_by_image_hash_from_image(res);
									ui.close_menu();
								}
							});

							ui.vertical(|ui|{
								ui.label(format!("Filename: {}", res.filename));
								ui.label(format!("Path: {}", res.path));
								ui.label(format!("Similarity: {}", 1.0f64 / (1.0f64+res.distance_from_query.unwrap_or(1e10f64))));
								ui.label(format!("Size: {}x{}", res.resolution.0, res.resolution.1));
							});
						});
					});
				});
			});
	}
}