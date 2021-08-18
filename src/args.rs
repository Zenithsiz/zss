//! Args

// Imports
use anyhow::Context;
use clap::{App as ClapApp, Arg as ClapArg};
use std::{path::PathBuf, time::Duration};

/// Args
pub struct Args {
	/// Window id
	pub window_id: u64,

	/// Duration
	pub duration: Duration,

	/// Images directory
	pub images_dir: PathBuf,

	/// Fade
	pub fade: f32,

	/// Image backlog
	pub image_backlog: usize,

	/// Mode
	pub mode: Mode,
}

/// Mode
pub enum Mode {
	/// Single image
	Single,

	/// Grid
	Grid {
		/// Width
		width: usize,

		/// Height
		height: usize,
	},
}

impl Args {
	/// Parses all arguments
	#[allow(clippy::too_many_lines)] // TODO: Refactor
	pub fn new() -> Result<Self, anyhow::Error> {
		const WINDOW_ID_STR: &str = "window-id";
		const IMAGES_DIR_STR: &str = "images-dir";
		const DURATION_STR: &str = "duration";
		const FADE_STR: &str = "fade";
		const IMAGE_BACKLOG_STR: &str = "image-backlog";
		const GRID_STR: &str = "grid";

		// Get all matches from cli
		let matches = ClapApp::new("Zss")
			.version("1.0")
			.author("Filipe Rodrigues <filipejacintorodrigues1@gmail.com>")
			.about("Displays a scrolling wallpaper with Multiple images")
			.arg(
				ClapArg::with_name(WINDOW_ID_STR)
					.help("The window id")
					.long_help("An `X` window id. Typically obtained from `xwinwrap`")
					.takes_value(true)
					.required(true)
					.long("window-id")
					.short("w")
					.index(1),
			)
			.arg(
				ClapArg::with_name(IMAGES_DIR_STR)
					.help("Images Directory")
					.long_help("Path to directory with images. Non-images will be ignored.")
					.takes_value(true)
					.required(true)
					.long("images-dir")
					.short("i")
					.index(2),
			)
			.arg(
				ClapArg::with_name(DURATION_STR)
					.help("Duration (in seconds) of each image")
					.long_help("Duration, in seconds, each image will take up on screen, including during fading.")
					.takes_value(true)
					.long("duration")
					.short("d")
					.default_value("30"),
			)
			.arg(
				ClapArg::with_name(FADE_STR)
					.help("Fade percentage (0.5 .. 1.0)")
					.long_help("Percentage, from 0.5 to 1.0, of when to start fading the image during it's display.")
					.takes_value(true)
					.long("fade")
					.short("f")
					.default_value("0.8"),
			)
			.arg(
				ClapArg::with_name(IMAGE_BACKLOG_STR)
					.help("Image backlog")
					.long_help("Number of images to keep loaded, aside from 2/3 that must be always loaded.")
					.takes_value(true)
					.long("backlog")
					.short("b")
					.default_value("0"),
			)
			.arg(
				ClapArg::with_name(GRID_STR)
					.help("Grid")
					.long_help("Displays a grid of images, as `{width}x{height}`")
					.takes_value(true)
					.long("grid"),
			)
			.get_matches();

		let window_id = matches.value_of(WINDOW_ID_STR).expect("Required argument was missing");
		log::info!("Found window id {window_id}");
		anyhow::ensure!(window_id.starts_with("0x"), "Window id didn't start with `0x`");
		let window_id = u64::from_str_radix(&window_id[2..], 16).context("Unable to parse window id")?;

		let duration = matches
			.value_of(DURATION_STR)
			.expect("Argument with default value was missing");
		let duration = duration.parse().context("Unable to parse duration")?;
		let duration = Duration::from_secs_f32(duration);

		let images_dir = PathBuf::from(
			matches
				.value_of_os(IMAGES_DIR_STR)
				.expect("Required argument was missing"),
		);

		let fade = matches
			.value_of(FADE_STR)
			.expect("Argument with default value was missing");
		let fade = fade.parse().context("Unable to parse fade")?;
		anyhow::ensure!((0.5..=1.0).contains(&fade), "Fade must be within 0.5 .. 1.0");

		let image_backlog = matches
			.value_of(IMAGE_BACKLOG_STR)
			.expect("Argument with default value was missing");
		let image_backlog = image_backlog.parse().context("Unable to parse image backlog")?;

		let mode = match matches.value_of(GRID_STR) {
			Some(grid) => {
				let (width, height) = grid
					.split_once('x')
					.context("Grid must be of the format `{width}x{height}`")?;
				let width = width.trim().parse().context("Unable to parse grid width")?;
				let height = height.trim().parse().context("Unable to parse grid height")?;

				Mode::Grid { width, height }
			},
			None => Mode::Single,
		};

		Ok(Self {
			window_id,
			duration,
			images_dir,
			fade,
			image_backlog,
			mode,
		})
	}
}
