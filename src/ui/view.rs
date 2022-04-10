use std::ops::Mul;
use crate::{AppTab, MainApp};
use crate::ui::load_image_from_path;
use eframe::{egui, epi};
use eframe::egui::{Context, TextureHandle, Ui};
use std::path::Path;

// Still TODO:
// If the image isn't found or can't be read, this will try to re-load it every frame.
// The image is just displayed plainly.  No ability to change anything or zoom or show meta.
// No errors shown if an image can't be displayed.

pub fn view_panel(
	app_state: &mut MainApp,
	ui: &mut egui::Ui
) {
	if app_state.engine.is_none() {
		ui.label("To search for an image, make sure a DB is loaded and folders have been indexed.");
		return;
	}

	if app_state.selected_image.is_none() {
		ui.label("No image selected.  Right click an image and choose 'view' in the search results.");
		return;
	}
	let selected_image = app_state.selected_image.as_ref().unwrap();

	// An image may be loaded that doesn't match with what's in the selected image.
	// That is to say, we might have switched the selected without clearing it.
	if app_state.full_image_path != selected_image.path {
		app_state.full_image_path = selected_image.path.clone();
		app_state.full_image = {
			if let Ok(img) = load_image_from_path(Path::new(&app_state.full_image_path)) {
				Some(ui.ctx().load_texture(app_state.full_image_path.clone(), img))
			} else {
				None
			}
		};
		//app_state.full_image = Some(RetainedImage::)
	}

	ui.vertical(|ui|{
		ui.label(format!("Filename: {}", selected_image.filename));
		ui.label(format!("Path: {}", selected_image.path));
		ui.label(format!("Size: {}x{}", selected_image.resolution.0, selected_image.resolution.1));
		ui.label("EXIF Tags:");
		dbg!(&selected_image.tags);
		for (k, v) in &selected_image.tags {
			ui.label(format!("{}: {}", k, v));
		}
	});

	if let Some(tex) = &app_state.full_image {
		egui::ScrollArea::both()
			.auto_shrink([false, false])
			.show(ui, |ui| {
				// Show the image:
				//ui.add(egui::Image::new(texture, texture.size_vec2()));
				// Same:
				ui.image(tex, tex.size_vec2()*1.0f32);
			});
	}
}