//! Images

// Imports
use anyhow::Context;
use image::{GenericImageView, ImageBuffer, Rgba};
use rand::prelude::SliceRandom;
use std::sync::mpsc;

/// Images
pub struct Images {
	/// Images channel
	receiver: mpsc::Receiver<ImageBuffer<Rgba<u8>, Vec<u8>>>,
}

impl Images {
	/// Loads all images' paths
	pub fn new(window_width: u32, window_height: u32) -> Result<Self, anyhow::Error> {
		// Get all paths and shuffle them
		let mut paths = std::fs::read_dir("/home/filipe/.wallpaper/active")
			.context("Unable to read directory")?
			.map(|entry| entry.map(|entry| entry.path()))
			.collect::<Result<Vec<_>, _>>()
			.context("Unable to read entries")?;
		log::info!("Found {} images", paths.len());

		// Create the channel
		let (sender, receiver) = mpsc::sync_channel(0);

		// Start loading them in a background thread
		std::thread::spawn(move || {
			loop {
				// Shuffles all paths
				paths.shuffle(&mut rand::thread_rng());

				// Then load them all
				// TODO: Maybe cache a few between shuffles?
				for path in &paths {
					let res: Result<_, anyhow::Error> = try {
						// Open the image, resizing it to it's max
						let image_reader = image::io::Reader::open(path)
							.context("Unable to open image")?
							.with_guessed_format()
							.context("Unable to parse image")?;
						image_reader.decode().context("Unable to decode image")?.flipv()
					};

					let image = match res {
						Ok(image) => image,
						Err(err) => {
							log::warn!("Unable to load {path:?}: {err:?}");
							continue;
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
						return;
					};
				}
			}
		});


		Ok(Self { receiver })
	}

	/// Returns the next image
	pub fn next_image(&self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
		self.receiver.recv().expect("Unable to get next image")
	}
}
