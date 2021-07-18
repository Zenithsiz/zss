//! Args

// Imports
use anyhow::Context;
use clap::{App as ClapApp, Arg as ClapArg};
use std::time::Duration;

/// Args
pub struct Args {
	/// Window id
	pub window_id: u64,

	/// Duration
	pub duration: Duration,
}

impl Args {
	/// Parses all arguments
	pub fn new() -> Result<Self, anyhow::Error> {
		const WINDOW_ID_STR: &str = "window-id";
		const DURATION_STR: &str = "duration";

		// Get all matches from cli
		let matches = ClapApp::new("Zss")
			.version("0.0")
			.author("Filipe [...] <[...]@gmail.com>")
			.about("Displays a wallpaper")
			.arg(
				ClapArg::with_name(WINDOW_ID_STR)
					.help("The window id")
					.required(true)
					.index(1),
			)
			.arg(
				ClapArg::with_name(DURATION_STR)
					.help("Duration of each one")
					.takes_value(true)
					.long("duration")
					.short("d"),
			)
			.get_matches();

		let window_id = matches.value_of(WINDOW_ID_STR).expect("Required argument was missing");
		log::info!("Found window id {window_id}");
		anyhow::ensure!(window_id.starts_with("0x"), "Window id didn't start with `0x`");
		let window_id = u64::from_str_radix(&window_id[2..], 16).context("Unable to parse window id")?;

		let duration = matches.value_of(DURATION_STR);
		let duration = duration
			.map(str::parse)
			.transpose()
			.context("Invalid duration")?
			.map_or(Duration::from_secs(30), Duration::from_secs_f32);

		Ok(Self { window_id, duration })
	}
}
