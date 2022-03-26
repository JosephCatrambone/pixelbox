use crate::indexed_image::IndexedImage;
use eframe::{epi, egui::{self, Ui, TextureId}};
use std::collections::HashMap;

// TODO: Maybe move the thumbnail cache fill out of this method.

pub fn image_grid(ui:&mut Ui, frame: &mut epi::Frame, results:Vec<IndexedImage>, thumbnail_cache: &mut HashMap::<i64, TextureId>, thumbnail_size:(f32, f32)) {
	let num_results = results.len();
	let num_columns = (ui.available_width() / thumbnail_size.0).max(1.0f32) as usize;
	//let num_rows = num_results / num_columns;

	egui::Grid::new("image_result_grid")
		.striped(false)
		.min_col_width(thumbnail_size.0)
		.max_col_width(thumbnail_size.0)
		.show(ui, |ui| {
			for row in 0..(num_results / num_columns) {
				for col in 0..num_columns {
					let res = &results[col + row * num_columns];
					//ui.add(egui::Image::new(my_texture_id, [640.0, 480.0]));
					//ui.image(tex_id, [res.thumbnail_resolution.0 as f32, res.thumbnail_resolution.1 as f32]);
					//ui.label(format!("Img: {}", &results[col + row*num_columns].filename));
					// To handle right click:
					//ui.button("Test").secondary_clicked()
				}
				ui.end_row();
			}
		});
}