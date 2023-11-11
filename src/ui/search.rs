use crate::{AppTab, MainApp};
//use crate::engine::Engine;
use crate::ui::{fetch_or_generate_thumbnail, paginate};
use eframe::{egui, NativeOptions};
use eframe::egui::{Context, DroppedFile, TextureHandle, Ui};
use rfd;
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
		// Search by image _buttons_.
		if ui.button("Search by Image").clicked() {
			if let Some(file_path) = rfd::FileDialog::new().pick_file() {
				app_state.engine.as_mut().unwrap().query_by_image_hash_from_file(Path::new(&file_path))
				//app_state.engine.as_mut().unwrap().query(&format!("similar:{}", file_path.to_str().unwrap()));
			}
		}

		// Search by image drag+drop support.
		if let Some(images) = detect_files_being_dropped(ui.ctx()) {
			app_state.engine.as_mut().unwrap().query_by_image_hash_from_file(images.first().unwrap().path.as_ref().unwrap())
			//app_state.engine.as_mut().unwrap().query(&format!("similar:{}", images.first().unwrap().path.unwrap().to_str().unwrap()));
		}
		
		// Universal Search
		if ui.text_edit_singleline(&mut app_state.search_text).changed() && app_state.search_text.len() > app_state.search_text_min_length as usize {
			let query_success = app_state.engine.as_mut().unwrap().query(&app_state.search_text.clone());
			if let Err(q) = query_success {
				app_state.query_error = q.to_string();
			} else {
				app_state.query_error = "".to_string();
			}
			//app_state.engine.as_mut().unwrap().query_by_image_name(&app_state.search_text.clone())
		}
	});

	// Show parsing errors in query.
	if !app_state.query_error.is_empty() {
		ui.label(&app_state.query_error);
	}

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
							
							// Note: thumbnail size != image size.  We might want to show them off as larger or smaller.
							ui.image(&tex_id).context_menu(|ui|{
								if ui.button("Open").clicked() {
									//let _ = std::process::Command::new("open").arg(&res.path).output();
									open::that(&res.path);
									ui.close_menu();
								}
								if ui.button("Open in View Tab").clicked() {
									//let _ = std::process::Command::new("open").arg(&res.path).output();
									app_state.selected_image = Some(res.clone());
									app_state.active_tab = AppTab::View;
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
								ui.label(format!("Distance: {}", res.distance_from_query.unwrap_or(1e3f64)));
								ui.label(format!("Size: {}x{}", res.resolution.0, res.resolution.1));
							});
						});
					});
				});
			});
	}
}

// Flagrantly stolen from the drag-and-drop documentation:
// https://github.com/emilk/egui/blob/master/eframe/examples/file_dialog.rs#L67
fn detect_files_being_dropped(ctx: &egui::Context) -> Option<Vec<DroppedFile>> {
	// TODO: Needs updating for the latest version of egui.
	// Preview hovering files:
	/*
	if !ctx.input().raw.hovered_files.is_empty() {
		let mut text = "Dropping files:\n".to_owned();
		for file in &ctx.input().raw.hovered_files {
			if let Some(path) = &file.path {
				text += &format!("\n{}", path.display());
			} else if !file.mime.is_empty() {
				text += &format!("\n{}", file.mime);
			} else {
				text += "\n???";
			}
		}

		let painter =
			ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("file_drop_target")));

		let screen_rect = ctx.input().screen_rect();
		painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(192));
		painter.text(
			screen_rect.center(),
			egui::Align2::CENTER_CENTER,
			text,
			egui::TextStyle::Heading.resolve(&ctx.style()),
			egui::Color32::WHITE,
		);
	}
	*/

	// Collect dropped files:
	let mut files = Vec::<DroppedFile>::new();
	ctx.input(|i| {
		if !i.raw.dropped_files.is_empty() {
			files.append(&mut i.raw.dropped_files.clone());
		}
	});
	if !files.is_empty() {
		return Some(files);
	}
	
	None
}