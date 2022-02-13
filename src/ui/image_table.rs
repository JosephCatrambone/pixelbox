
use crate::indexed_image::IndexedImage;
use super::thumbnail_to_egui_element;
use eframe::{epi, egui::{self, Ui, TextureId}};
use std::collections::HashMap;
use open;

pub fn image_table(ui:&mut Ui, ctx: &egui::Context, frame: &epi::Frame, results:Vec<IndexedImage>, thumbnail_cache: &mut HashMap::<i64, TextureId>) {
	ui.vertical(|ui|{
		results.iter().for_each(|res|{
			ui.horizontal(|ui|{
				let tex_id = match thumbnail_cache.get(&res.id) {
					Some(tid) => *tid,
					None => {
						let tid = thumbnail_to_egui_element(res, ctx, frame);
						thumbnail_cache.insert(res.id, tid);
						tid
					}
				};
				ui.image(tex_id, [res.thumbnail_resolution.0 as f32, res.thumbnail_resolution.1 as f32]).context_menu(|ui|{
					if ui.button("Open").clicked() {
						//let _ = std::process::Command::new("open").arg(&res.path).output();
						open::that(&res.path);
						ui.close_menu();
					}
					if ui.button("Search for Similar").clicked() {
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
}