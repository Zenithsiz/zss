//! Images

// Imports
use crate::window::Window;
use anyhow::Context;
use image::{imageops::FilterType, GenericImageView, ImageBuffer, Rgba};
use rand::prelude::SliceRandom;
use std::{
	path::{Path, PathBuf},
	rc::Rc,
	sync::mpsc,
	time::Duration,
};

/// Images
// TODO: Maybe notice when new paths are added to the folder somehow?
pub struct Images {
	/// Paths
	paths: Vec<PathBuf>,

	/// Image backlog
	image_backlog: usize,

	/// Receiver end for the image loading.
	receiver: mpsc::Receiver<ImageBuffer<Rgba<u8>, Vec<u8>>>,

	/// Window
	window: Rc<Window>,
}

impl Images {
	/// Loads all images' paths
	pub fn new(images_dir: impl AsRef<Path>, image_backlog: usize, window: Rc<Window>) -> Result<Self, anyhow::Error> {
		// Get all paths
		let paths = load_paths(&images_dir)?;

		// Start loading them in a background thread
		let (sender, receiver) = mpsc::sync_channel(image_backlog);
		let thread_paths = paths.clone();
		let window_size = window.size();
		std::thread::spawn(move || self::image_loader(thread_paths, window_size, sender));


		Ok(Self {
			paths,
			image_backlog,
			receiver,
			window,
		})
	}

	/// Returns the next image
	pub fn next_image(&mut self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
		// Current timeout if we need to retry the thread
		let mut cur_timeout = 1.0;

		loop {
			match self.receiver.recv() {
				// if we got it, return
				Ok(image) => return image,

				// If unable to, wait and increase the timeout
				Err(_) => {
					// Wait the timeout
					log::info!("Loading thread died, waiting {cur_timeout} seconds before restarting thread");
					std::thread::sleep(Duration::from_secs_f32(cur_timeout));

					// Double it, up to 30 seconds
					cur_timeout *= 2.0;
					cur_timeout = cur_timeout.min(30.0);

					// Re-spawn the thread and set our new receiver
					let (sender, receiver) = mpsc::sync_channel(self.image_backlog);
					let paths = self.paths.clone();
					let window_size = self.window.size();
					std::thread::spawn(move || self::image_loader(paths, window_size, sender));
					self.receiver = receiver;
				},
			}
		}
	}
}

/// Loads all paths from a path
fn load_paths(images_dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, anyhow::Error> {
	std::fs::read_dir(images_dir)
		.context("Unable to read directory")?
		.map(|entry| entry.map(|entry| entry.path()))
		.collect::<Result<Vec<_>, _>>()
		.context("Unable to read entries")
}

/// Image loader to run in a background thread
#[allow(clippy::needless_pass_by_value)] // It's better for this function to own the sender
fn image_loader(
	mut paths: Vec<PathBuf>, [window_width, window_height]: [u32; 2],
	sender: mpsc::SyncSender<ImageBuffer<Rgba<u8>, Vec<u8>>>,
) {
	log::info!("Found {} images", paths.len());

	let mut should_quit = false;
	while !should_quit {
		// Shuffles all paths
		paths.shuffle(&mut rand::thread_rng());

		// If we're out of paths, quit
		if paths.is_empty() {
			log::warn!("No images found, quitting loading thread");
			return;
		}

		// Then load them all and send them
		paths.drain_filter(|path| {
			if should_quit {
				return false;
			}

			// Try to load it
			let res: Result<_, anyhow::Error> = try {
				// Open the image, resizing it to it's max
				let image_reader = image::io::Reader::open(&path)
					.context("Unable to open image")?
					.with_guessed_format()
					.context("Unable to parse image")?;
				image_reader.decode().context("Unable to decode image")?.flipv()
			};

			let image = match res {
				Ok(image) => image,
				// If we got an error, remove this from the paths
				Err(err) => {
					log::warn!("Unable to load {path:?}: {err:?}");
					return true;
				},
			};
			log::info!("Loaded {path:?}");

			// Then resize it to it's max size, taking into account the zooming we do
			let (resize_width, resize_height) = match image.width() >= image.height() {
				true => match image.height() >= window_height {
					true => (image.width() * window_height / image.height(), window_height),
					false => (image.width(), image.height()),
				},
				false => match image.width() >= window_width {
					true => (window_width, image.height() * window_width / image.width()),
					false => (image.width(), image.height()),
				},
			};
			let image = image
				.resize_exact(resize_width, resize_height, FilterType::Lanczos3)
				.to_rgba8();

			// Then send it and quit if we're done
			if sender.send(image).is_err() {
				should_quit = true;
			};

			false
		});
	}
}
