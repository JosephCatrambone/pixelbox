## PixelBox
#### A desktop image search and indexing tool.

---

![Demo Screenshot - Filename Search](https://github.com/JosephCatrambone/pixelbox/blob/main/.github/images/demo_text_search.png?raw=true)

PixelBox is still pre-alpha.  Database schema and feature prioritization are subject to change.

### Features
* Cross-platform (Windows, Linux, MacOS) and FOSS
* Search across filenames and exif tags
* Drag and drop search for visually similar images
* Fast parallel indexing of images
* User-moddable image similarity engine (!)
* Portable and inspectable database format

### Technologies
* Rust as the primary language (with egui and tract-onnx)
* SQLite as a storage medium for the image database
* Torch for training the image similarity model
* ONNX for running the similarity model

### TODOs
* Compress thumbnails in database
* Index inside of zip files
* Don't search on fewer than two characters
* Remove from index on folder clear
* Better similarity search
* Start removing those unwraps
* OCR for images (search on text in images)
* Editable tags
* Face search
* Search on image contents in plaintext
* Watched directories via notify crate

### Project Structure

* .github - Links to demo pictures for readme and, eventually, CI/GitHub Action build scripts
* models - The final ONNX files to be used by the application for visual similarity
* resources - Non-shipped experiment logs and python training files
* src - The main application code
  * image_hashes - Wrappers for different image hashing methods
  * ui - Code for each of the major UI panels like search view, folder view, etc.

### Using Your Own Image Hash

PixelBox's search uses the cosine distance between byte-quantified n-dimensional floats.
For example, if you represent your image as [-1.0, 1.0, 0.0, 0.1] then this will be mapped to a 4-byte vector of [0x00, 0xFF, 0x80, 0x8C].

There are two ways to use your own image hash methods:

1) Replace the encoder_cpu.onnx file with your own trained model.  The inputs should be channel-first 128x128 RGB images and the outputs should be a 1D vector of floats between -1 and 1.  See image_hashes/convnet.rs for constraints.
2) Replace the 'hash' in the 'semantic_hash' table of your database.  This should be an array of u8s as described above.  You will not be able to drag-and-drop images for search if using this approach, but after finding a seed image you can right-click and do 'find similar'.