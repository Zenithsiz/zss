//! Images

// Imports
use crate::window::Window;
use anyhow::Context;
use image::{GenericImageView, ImageBuffer, Rgba};
use rand::prelude::SliceRandom;
use std::{path::Path, sync::mpsc};

/// Images
pub struct Images {
	/// Images channel
	receiver: mpsc::Receiver<ImageBuffer<Rgba<u8>, Vec<u8>>>,
}

impl Images {
	/// Loads all images' paths
	pub fn new(images_dir: impl AsRef<Path>, window: &Window) -> Result<Self, anyhow::Error> {
		// Get all paths and shuffle them
		let mut paths = std::fs::read_dir(images_dir)
			.context("Unable to read directory")?
			.map(|entry| entry.map(|entry| entry.path()))
			.collect::<Result<Vec<_>, _>>()
			.context("Unable to read entries")?;
		log::info!("Found {} images", paths.len());

		// Create the channel
		let (sender, receiver) = mpsc::sync_channel(0);

		// Start loading them in a background thread
		let [window_width, window_height] = window.size();
		std::thread::spawn(move || {
			let mut should_quit = false;
			while !should_quit {
				// Shuffles all paths
				paths.shuffle(&mut rand::thread_rng());

				// If we have no paths, panic
				// TODO: Better solution
				if paths.is_empty() {
					panic!("No more paths are left to display");
				}

				// Then load them all
				// TODO: Maybe cache a few between shuffles?
				paths.drain_filter(|path| {
					if should_quit {
						return false;
					}

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

					let image = image.thumbnail_exact(resize_width, resize_height).to_rgba8();

					if sender.send(image).is_err() {
						should_quit = true;
					};

					false
				});
			}
		});


		Ok(Self { receiver })
	}

	/// Returns the next image
	pub fn next_image(&self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
		self.receiver.recv().expect("Unable to get next image")
	}
}
