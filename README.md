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
