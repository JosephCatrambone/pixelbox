
mod engine;
mod image_hashes;
mod indexed_image;

use engine::Engine;
use std::path::Path;

fn main() {
	let mut engine = Engine::new(Path::new("test.db"));
	engine.add_tracked_folder("./test_resources/*.*".to_string());
	engine.start_reindexing();
}