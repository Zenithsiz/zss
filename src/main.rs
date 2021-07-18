//! Zss

// Features
#![feature(
	raw_ref_op,
	format_args_capture,
	atomic_mut_ptr,
	bindings_after_at,
	destructuring_assignment,
	maybe_uninit_uninit_array,
	maybe_uninit_array_assume_init,
	try_blocks,
	drain_filter
)]
#![warn(unsafe_op_in_unsafe_fn)]

// Modules
mod args;
mod images;
mod program;
mod texture;
mod uvs;
mod vao;
mod window;

// Imports
use crate::{images::Images, program::Program, vao::Vao};
use anyhow::Context;
use args::Args;
use image::{ImageBuffer, Rgba};
use texture::Texture;
use uvs::Uvs;

fn main() -> Result<(), anyhow::Error> {
	// Initialize logger
	simplelog::TermLogger::init(
		log::LevelFilter::Info,
		simplelog::Config::default(),
		simplelog::TerminalMode::Stderr,
		simplelog::ColorChoice::Auto,
	)
	.expect("Unable to initialize logger");

	// Get arguments
	let args = Args::new().context("Unable to retrieve arguments")?;

	// Then create the window state
	let mut window_state =
		unsafe { window::Window::from_window_id(args.window_id) }.context("Unable to initialize open-gl context")?;
	let [window_width, window_height] = window_state.size();

	// Load all images
	let images = Images::new(&args.images_dir, window_width, window_height).context("Unable to load images")?;

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

	// Main Loop
	let mut uvs = None;
	let mut progress = 0.0;
	loop {
		// Check for events
		window_state.process_events();

		// Get the uvs
		let new_uvs = || {
			self::setup_new_image(
				images.next_image(),
				&tex,
				&vao,
				window_width,
				window_height,
				rand::random(),
			)
			.context("Unable to get new image")
		};
		let uvs = match uvs.as_mut() {
			// If we have none, or the current image ended
			None => uvs.insert(new_uvs()?),
			Some(_) if progress >= 1.0 => {
				progress = 0.0;
				uvs.insert(new_uvs()?)
			},
			Some(uvs) => uvs,
		};

		// Clear
		unsafe {
			gl::ClearColor(0.0, 0.0, 0.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
		}

		// And draw
		program.with_using(|| {
			// Update the texture offset
			let tex_offset = uvs.offset(progress);
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

		progress += 1.0 / 60.0 / args.duration.as_secs_f32();

		// Finally swap buffers
		window_state.swap_buffers();
	}
}

/// Opens and setups a new image
#[allow(clippy::type_complexity)] // TODO
fn setup_new_image(
	image: ImageBuffer<Rgba<u8>, Vec<u8>>, tex: &Texture, vao: &Vao, window_width: u32, window_height: u32,
	swap_dir: bool,
) -> Result<Uvs, anyhow::Error> {
	// Update our texture
	tex.update(&image);

	// Then create the uvs
	let uvs = Uvs::new(
		image.width() as f32,
		image.height() as f32,
		window_width as f32,
		window_height as f32,
		swap_dir,
	);

	// And update the vertices
	let start_uvs = uvs.start();
	#[rustfmt::skip]
	let vertices: [f32; 16] = [
		// Vertex  /   Uvs
		-1.0, -1.0,  0.0         , 0.0,
		 1.0, -1.0,  start_uvs[0], 0.0,
		-1.0,  1.0,  0.0         , start_uvs[1],
		 1.0,  1.0,  start_uvs[0], start_uvs[1],
	];
	vao.update_vertices(&vertices);

	Ok(uvs)
}
