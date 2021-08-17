//! Images

// Imports
use crate::window::Window;
use anyhow::Context;
use image::{imageops::FilterType, GenericImageView, ImageBuffer, Rgba};
use num_rational::Ratio;
use rand::prelude::SliceRandom;
use std::{
	cmp::Ordering,
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

	/// Returns the next image, waiting if not yet available
	pub fn next_image(&mut self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
		// Current timeout if we need to retry the thread
		let mut cur_timeout = 1.0;

		loop {
			match self.receiver.recv() {
				// if we got it, return
				Ok(image) => return image,

				// If unable to, wait and increase the timeout
				Err(_) => self.on_disconnect(&mut cur_timeout),
			}
		}
	}

	/// Returns the next image, returning `None` if not yet loaded
	pub fn try_next_image(&mut self) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
		// Current timeout if we need to retry the thread
		let mut cur_timeout = 1.0;

		loop {
			match self.receiver.try_recv() {
				// if we got it, return it
				Ok(image) => return Some(image),

				// If it wasn't ready, return `None`
				Err(mpsc::TryRecvError::Empty) => return None,

				// If unable to, wait and increase the timeout
				Err(mpsc::TryRecvError::Disconnected) => self.on_disconnect(&mut cur_timeout),
			}
		}
	}

	/// Function for retrying on disconnect
	fn on_disconnect(&mut self, cur_timeout: &mut f32) {
		// Wait the timeout
		log::info!("Loading thread died, waiting {cur_timeout} seconds before restarting thread");
		std::thread::sleep(Duration::from_secs_f32(*cur_timeout));

		// Double it, up to 30 seconds
		*cur_timeout *= 2.0;
		*cur_timeout = cur_timeout.min(30.0);

		// Re-spawn the thread and set our new receiver
		let (sender, receiver) = mpsc::sync_channel(self.image_backlog);
		let paths = self.paths.clone();
		let window_size = self.window.size();
		std::thread::spawn(move || self::image_loader(paths, window_size, sender));
		self.receiver = receiver;
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
				image_reader.decode().context("Unable to decode image")?
			};

			let image = match res {
				Ok(image) => image,
				// If we got an error, remove this from the paths
				Err(err) => {
					log::warn!("Unable to load {path:?}: {err:?}");
					return true;
				},
			};
			let (image_width, image_height) = (image.width(), image.height());
			log::info!("Loaded {path:?} ({image_width}x{image_height})");

			// Get the aspect ratios
			let image_aspect_ratio = Ratio::new(image_width, image_height);
			let window_aspect_ratio = Ratio::new(window_width, window_height);

			// Check what direction we'll be scrolling with this image
			let scroll_dir = match (image_width.cmp(&image_height), window_width.cmp(&window_height)) {
				// If they're both square, no scrolling occurs
				(Ordering::Equal, Ordering::Equal) => ScrollDir::None,

				// Else if the window is tall and the window is wide, it must scroll vertically
				(Ordering::Less | Ordering::Equal, Ordering::Greater | Ordering::Equal) => ScrollDir::Vertically,

				// Else if the window is wide and the window is tall, it must scroll horizontally
				(Ordering::Greater | Ordering::Equal, Ordering::Less | Ordering::Equal) => ScrollDir::Horizontally,

				// Else we need to check the aspect ratio
				(Ordering::Less, Ordering::Less) | (Ordering::Greater, Ordering::Greater) => {
					match image_aspect_ratio.cmp(&window_aspect_ratio) {
						// If the image is wider than the screen, we'll scroll horizontally
						Ordering::Greater => ScrollDir::Horizontally,

						// Else if the image is taller than the screen, we'll scroll vertically
						Ordering::Less => ScrollDir::Vertically,

						// Else if they're equal, no scrolling occurs
						Ordering::Equal => ScrollDir::None,
					}
				},
			};

			match scroll_dir {
				ScrollDir::Vertically => log::info!("Scrolling image vertically"),
				ScrollDir::Horizontally => log::info!("Scrolling image horizontally"),
				ScrollDir::None => log::info!("Not scrolling image"),
			}

			let resize_size = match scroll_dir {
				// If we're scrolling vertically, resize if the image width is larger than the window width
				ScrollDir::Vertically if image_width > window_width => {
					Some((window_width, (window_width * image_height) / image_width))
				},

				// If we're scrolling horizontally, resize if the image height is larger than the window height
				ScrollDir::Horizontally if image_height > window_height => {
					Some(((window_height * image_width) / image_height, window_height))
				},

				// If we're not doing any scrolling and the window is smaller, resize the image to screen size
				// Note: Since we're not scrolling, we know aspect ratio is the same and so
				//       we only need to check the width.
				ScrollDir::None if image_width > window_width => Some((window_width, window_height)),

				// Else don't do any scrolling
				_ => None,
			};

			// Resize if required
			let image = match resize_size {
				Some((resize_width, resize_height)) => {
					log::info!("Resizing from {image_width}x{image_height} to {resize_width}x{resize_height}",);
					image.resize_exact(resize_width, resize_height, FilterType::Lanczos3)
				},
				None => {
					log::info!("Not resizing");
					image
				},
			};

			// Then flip the image vertically and get it as rgba8
			let image = image.flipv().to_rgba8();

			// Then send it and quit if we're done
			if sender.send(image).is_err() {
				should_quit = true;
			};

			false
		});
	}
}

/// Image scrolling direction
enum ScrollDir {
	Vertically,
	Horizontally,
	None,
}
