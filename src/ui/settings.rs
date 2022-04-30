use crate::{AppTab, MainApp};
use eframe::{egui, epi, NativeOptions};
use eframe::egui::{Context, DroppedFile, TextureHandle, Ui};

pub fn settings_panel(
	app_state: &mut MainApp,  // We will need this eventually.
	ui: &mut egui::Ui
) {
	ui.vertical(|ui|{
		ui.checkbox(&mut app_state.dark_mode, "Dark Mode");
		ui.add(egui::Slider::new(&mut app_state.search_text_min_length, 0..=255).text("Minimum Search Length")).on_hover_text("A search is automatically run when at least this many characters are entered into the search bar.  Be wary that 0 (match any letter) could slow down performance.");
		ui.add(egui::Slider::new(&mut app_state.thumbnail_size, 0..=255).text("Thumbnail Size"));

		if let Some(engine) = &mut app_state.engine {
			ui.add(egui::Slider::new(&mut engine.max_search_results, 0..=10000).text("Max Search Results")).on_hover_text("How many results will be shown during a search.  A high number will use more memory and may take longer to run.");
			ui.add(egui::Slider::new(&mut engine.max_distance_from_query, 0.0..=1.0).text("Max Query Dissimilarity")).on_hover_text("How dissimilar can an image be before it is removed from the results?  At 0, images must be identical to be shown.  At 1, unrelated images will be shown.");
		} else {
			// Honestly, this should never happen, but let's be safe.
			ui.label("Max Search Results and Max Query Distance can be configured when a DB has been opened.");
		}

		// Configuration options to implement
		// Maybe search weights for similarity vector?
		// Reindex/refresh check increment (disable background auto-check to use zero CPU when not in focus)
		// Toggle always-refresh?
		
		//if ui.button("If you push this button nothing will happen").clicked() {}
		//if ui.text_edit_singleline(&mut app_state.search_text).changed() {}
	});
}
