use crate::{AppTab, MainApp};
//use crate::engine::Engine;
use eframe::{egui, epi, NativeOptions};
use eframe::egui::{Context, DroppedFile, TextureHandle, Ui};

pub fn settings_panel(
	app_state: &mut MainApp,  // We will need this eventually.
	ui: &mut egui::Ui
) {
	ui.vertical(|ui|{
		ui.label("Sorry -- this isn't built yet.  :(");
		
		// Configuration options to implement
		// Max search results
		// Max distance cutoff for similarity
		// Minimum number of characters before searching
		// Maybe search weights for similarity vector?
		// Reindex/refresh check increment (disable background auto-check to use zero CPU when not in focus)
		// Toggle always-refresh?
		// Dark mode toggle.
		
		if ui.button("If you push this button nothing will happen").clicked() {}

		//if ui.text_edit_singleline(&mut app_state.search_text).changed() {}
	});
}
