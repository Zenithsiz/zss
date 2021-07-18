//! Zss

// Features
#![feature(
	raw_ref_op,
	format_args_capture,
	atomic_mut_ptr,
	bindings_after_at,
	destructuring_assignment,
	maybe_uninit_uninit_array,
	maybe_uninit_array_assume_init
)]
#![warn(unsafe_op_in_unsafe_fn)]

// Modules
mod program;
mod texture;
mod vao;
mod window;

// Imports
use anyhow::Context;
use image::GenericImageView;
use rand::prelude::SliceRandom;
use std::path::Path;
use texture::Texture;

use crate::{program::Program, vao::Vao};

fn main() -> Result<(), anyhow::Error> {
	// Initialize logger
	simplelog::TermLogger::init(
		log::LevelFilter::Info,
		simplelog::Config::default(),
		simplelog::TerminalMode::Stderr,
		simplelog::ColorChoice::Auto,
	)
	.expect("Unable to initialize logger");

	// Get the window from arguments
	let window = std::env::args().nth(1).context("Must supply window id")?;
	log::info!("Found window id {window}");
	anyhow::ensure!(window.starts_with("0x"), "Window id didn't start with `0x`");
	let window = u64::from_str_radix(&window[2..], 16).context("Unable to parse window id")?;

	// Then create the window state
	let mut window_state =
		unsafe { window::Window::from_window_id(window) }.context("Unable to initialize open-gl context")?;
	let [window_width, window_height] = window_state.size();

	// Compile the shaders into a program
	let program = Program::new().context("Unable to create program")?;

	// Get the `tex_offset` location
	let tex_offset_location = program
		.uniform_location("tex_offset")
		.context("Unable to get uniform location")?;

	// Create the vao
	let vao = Vao::new();

	// Create the texture
	let tex = Texture::new();

	// Get all paths and shuffle them
	let mut paths = std::fs::read_dir("/home/filipe/.wallpaper/active")
		.context("Unable to read directory")?
		.map(|entry| entry.map(|entry| entry.path()))
		.collect::<Result<Vec<_>, _>>()
		.context("Unable to read entries")?;
	log::info!("Found {} images", paths.len());
	paths.shuffle(&mut rand::thread_rng());

	// Update the texture
	let mut cur_path = 0;
	let (mut dir, mut tex_offset, mut max) = self::setup_new_image(
		&paths[cur_path],
		&tex,
		&vao,
		window_width,
		window_height,
		rand::random(),
	)?;
	cur_path += 1;


	// Main Loop
	loop {
		// Check for events
		window_state.process_events();

		// Then draw
		unsafe {
			gl::ClearColor(0.0, 0.0, 0.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
		}

		program.with_using(|| {
			// Update the texture offset
			unsafe {
				gl::Uniform2f(tex_offset_location, tex_offset[0], tex_offset[1]);
			}

			// Then bind the vao and texture and draw
			vao.with_bound(|| {
				tex.with_bound(|| unsafe {
					gl::ActiveTexture(gl::TEXTURE0);
					gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null_mut());
				});
			});
		});

		tex_offset[0] += dir[0] * 0.002;
		tex_offset[1] += dir[1] * 0.002;

		if (max[0] != 0.0 && (tex_offset[0] <= 0.0 || tex_offset[0] >= max[0])) ||
			(max[1] != 0.0 && (tex_offset[1] <= 0.0 || tex_offset[1] >= max[1]))
		{
			// If we hit the end, shuffle again
			if cur_path >= paths.len() {
				paths.shuffle(&mut rand::thread_rng());
				cur_path = 0;
			}

			(dir, tex_offset, max) = self::setup_new_image(
				&paths[cur_path],
				&tex,
				&vao,
				window_width,
				window_height,
				rand::random(),
			)?;
			cur_path += 1;
		}

		// Then swap buffers
		window_state.swap_buffers();
	}
}

/// Opens and setups a new image
#[allow(clippy::type_complexity)] // TODO
fn setup_new_image(
	path: impl AsRef<Path>, tex: &Texture, vao: &Vao, window_width: u32, window_height: u32, swap_dir: bool,
) -> Result<([f32; 2], [f32; 2], [f32; 2]), anyhow::Error> {
	// Open the image, resizing it to it's max
	// TODO: Resize before opening with a custom generic image view
	let image_reader = image::io::Reader::open(path)
		.context("Unable to open image")?
		.with_guessed_format()
		.context("Unable to parse image")?;
	let image = image_reader.decode().context("Unable to decode image")?.flipv();

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

	// And update our texture
	tex.update(&image);

	// Then create the uvs
	let (uvs, dir, tex_offset, max) = self::create_uvs(
		image.width() as f32,
		image.height() as f32,
		window_width as f32,
		window_height as f32,
		swap_dir,
	);

	// And update the vertices
	#[rustfmt::skip]
	let vertices: [f32; 16] = [
		// Vertex  /   Uvs
		-1.0, -1.0,  0.0   , 0.0,
		 1.0, -1.0,  uvs[0], 0.0,
		-1.0,  1.0,  0.0   , uvs[1],
		 1.0,  1.0,  uvs[0], uvs[1],
	];
	vao.update_vertices(&vertices);

	Ok((dir, tex_offset, max))
}

/// Creates the uvs for an image
fn create_uvs(
	image_width: f32, image_height: f32, window_width: f32, window_height: f32, swap_dir: bool,
) -> ([f32; 2], [f32; 2], [f32; 2], [f32; 2]) {
	let (uvs, mut dir) = match image_width >= image_height {
		true => ([(window_width / image_width) / (window_height / image_height), 1.0], [
			1.0, 0.0,
		]),
		false => ([1.0, (window_height / image_height) / (window_width / image_width)], [
			0.0, 1.0,
		]),
	};
	let mut tex_offset: [f32; 2] = [0.0; 2];
	let max = [1.0 - uvs[0], 1.0 - uvs[1]];
	if swap_dir {
		dir[0] = -dir[0];
		dir[1] = -dir[1];
		tex_offset[0] = max[0];
		tex_offset[1] = max[1];
	}
	(uvs, dir, tex_offset, max)
}
