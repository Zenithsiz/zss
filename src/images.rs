//! Images

// Imports
use anyhow::Context;
use image::{imageops::FilterType, GenericImageView, ImageBuffer, Rgba};
use notify::Watcher;
use num_rational::Ratio;
use rand::prelude::SliceRandom;
use std::{
	cmp::Ordering,
	path::{Path, PathBuf},
	sync::mpsc::{self, RecvError, SendError},
	thread,
	time::Duration,
};

/// Image type
type Image = ImageBuffer<Rgba<u8>, Vec<u8>>;

/// Images
pub struct Images {
	/// Receiver end for the image loading.
	image_rx: mpsc::Receiver<Image>,

	/// Watcher
	_watcher: notify::RecommendedWatcher,
}

impl Images {
	/// Starts loading images in the background and returns the
	/// instance to retrieve them from.
	pub fn new(path: PathBuf, image_backlog: usize, window_size: [u32; 2]) -> Result<Self, anyhow::Error> {
		// Create the event channel
		let (event_tx, event_rx) = mpsc::channel();
		let mut existing_tx = event_tx.clone();

		// Then start the watcher and start watching the path
		let mut watcher =
			notify::watcher(event_tx, Duration::from_secs(2)).context("Unable to create directory watcher")?;
		watcher
			.watch(&path, notify::RecursiveMode::Recursive)
			.context("Unable to start watching directory")?;

		// Send existing files over the sender
		thread::spawn(move || {
			/// Sends all files in directory `dir`
			fn send_files_dir(path: &Path, tx: &mut mpsc::Sender<notify::DebouncedEvent>) -> Result<(), anyhow::Error> {
				for entry in std::fs::read_dir(path).context("Unable to read directory")? {
					let entry = entry.context("Unable to read directory entry")?;
					let file_type = entry.file_type().context("Unable to get entry file type")?;

					match file_type.is_dir() {
						// Recurse on directories
						true => send_files_dir(&entry.path(), tx).context("Unable to send files for sub-directory")?,

						// And send files + others
						false => {
							// Try to send it, or just quit else
							if tx.send(notify::DebouncedEvent::Create(entry.path())).is_err() {
								return Ok(());
							}
						},
					}
				}

				Ok(())
			}

			send_files_dir(&path, &mut existing_tx).expect("Unable to load exiting files");
		});


		// Start loading them in a background thread
		let (image_tx, image_rx) = mpsc::sync_channel(image_backlog);
		thread::spawn(move || {
			self::image_loader(event_rx, window_size, image_tx).expect("Background thread returned `Err`")
		});

		Ok(Self {
			image_rx,
			_watcher: watcher,
		})
	}

	/// Returns the next image, waiting if not yet available
	pub fn next_image(&mut self) -> Image {
		self.image_rx.recv().expect("Loading thread panicked")
	}

	/// Returns the next image, returning `None` if not yet loaded
	pub fn try_next_image(&mut self) -> Option<Image> {
		match self.image_rx.try_recv() {
			// if we got it, return it
			Ok(image) => Some(image),

			// If it wasn't ready, return `None`
			Err(mpsc::TryRecvError::Empty) => None,

			// If unable to, wait and increase the timeout
			Err(mpsc::TryRecvError::Disconnected) => panic!("Loading thread panicked"),
		}
	}
}

/// Image loader to run in a background thread
#[allow(clippy::needless_pass_by_value)] // It's better for this function to own the sender
fn image_loader(
	event_rx: mpsc::Receiver<notify::DebouncedEvent>, window_size: [u32; 2], image_tx: mpsc::SyncSender<Image>,
) -> Result<!, ImageLoaderError> {
	let mut paths = vec![];

	loop {
		// Shuffles all paths
		paths.shuffle(&mut rand::thread_rng());

		// Receives the next event, waiting if we're empty
		let next_event = |is_empty| match is_empty {
			true => {
				log::warn!("No images found, waiting for new files");
				Ok(Some(event_rx.recv()?))
			},
			false => match event_rx.try_recv() {
				Ok(path) => Ok(Some(path)),
				Err(mpsc::TryRecvError::Disconnected) => Err(mpsc::RecvError),
				Err(mpsc::TryRecvError::Empty) => Ok(None),
			},
		};

		// Check for new paths, or, if we're out, wait
		while let Some(event) = next_event(paths.is_empty()).map_err(ImageLoaderError::ReceiveEvent)? {
			// Note: No need to match `Remove`, the `drain_filter` below will remove it.
			// Note: On `Rename`, the original path will be removed by the `drain_filter` below
			match event {
				notify::DebouncedEvent::Create(path) | notify::DebouncedEvent::Rename(_, path) => {
					log::info!("Adding {path:?}");
					paths.push(path);
				},
				notify::DebouncedEvent::Error(err, path) => {
					log::warn!("Receiver error from directory watcher for {path:?}: {err}");
				},
				_ => (),
			}
		}


		// Then load them all and send them
		let mut send_err = None;
		paths.drain_filter(|path| {
			// If we have a sending error, just return
			if send_err.is_some() {
				return false;
			}

			// ELse try to load it
			let image = match self::load_img(path, window_size) {
				Ok(value) => value,
				Err(err) => {
					log::info!("Unable to load {path:?}: {err}");
					return true;
				},
			};

			// Then try to send it
			if let Err(err) = image_tx.send(image) {
				send_err = Some(err);
			}

			false
		});

		// If we got a send error, return Err
		if let Some(err) = send_err {
			return Err(ImageLoaderError::SendImage(err));
		}
	}
}

#[derive(Debug)]
enum ImageLoaderError {
	/// Unable to send image
	SendImage(SendError<Image>),

	/// Unable to receive fs event
	ReceiveEvent(RecvError),
}

/// Loads an image from a path
fn load_img(path: &Path, [window_width, window_height]: [u32; 2]) -> Result<Image, anyhow::Error> {
	// Try to open the image by guessing it's format
	let image_reader = image::io::Reader::open(&path)
		.context("Unable to open image")?
		.with_guessed_format()
		.context("Unable to parse image")?;
	let image = image_reader.decode().context("Unable to decode image")?;

	// Get it's width and aspect ratio
	let (image_width, image_height) = (image.width(), image.height());
	let image_aspect_ratio = Ratio::new(image_width, image_height);
	let window_aspect_ratio = Ratio::new(window_width, window_height);

	log::info!("Loaded {path:?} ({image_width}x{image_height})");

	// Then check what direction we'll be scrolling the image
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

	// Then get the size we'll be resizing to, if any
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

	// And resize if necessary
	let image = match resize_size {
		Some((resize_width, resize_height)) => {
			let reduction = 100.0 * (f64::from(resize_width) * f64::from(resize_height)) /
				(f64::from(image_width) * f64::from(image_height));

			log::info!(
				"Resizing from {image_width}x{image_height} to {resize_width}x{resize_height} ({reduction:.2}%)",
			);
			image.resize_exact(resize_width, resize_height, FilterType::Lanczos3)
		},
		None => {
			log::info!("Not resizing");
			image
		},
	};

	let image = image.flipv().to_rgba8();
	Ok(image)
}

/// Image scrolling direction
enum ScrollDir {
	Vertically,
	Horizontally,
	None,
}
