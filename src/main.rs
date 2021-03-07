
mod engine;
mod image_hashes;
mod indexed_image;
mod ui;

use engine::Engine;
use fltk::*;
use std::path::Path;
use std::time::Duration;


fn main() {
	let mut engine = if !db_file.exists() {
		let mut engine = Engine::new(&db_file);
		engine.add_tracked_folder("./test_resources/*.*".to_string());
		engine
	} else {
		Engine::open(&db_file)
	};
	engine.start_reindexing();

	let app = app::App::default()
		.with_scheme(app::Scheme::Gtk);
	let mut ui = ui::UserInterface::make_window();
	//ui.but.set_callback(move || {
	//	println!("Works!");
	//});
	app.run().unwrap();

	std::thread::sleep(Duration::from_millis(100));
	while engine.is_indexing_active() {
		println!("Waiting for indexing to finish.");
		std::thread::sleep(Duration::from_millis(100));
	}
	//std::fs::remove_file("test.db");
}