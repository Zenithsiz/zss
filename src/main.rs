//! Zss

// Features
#![feature(format_args_capture, try_blocks, drain_filter, never_type)]
// Warnings
#![warn(
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::pedantic,
	clippy::complexity,
	clippy::cargo,
	clippy::nursery
)]
#![warn(unsafe_op_in_unsafe_fn)]
// `match` can look better than `if` + `else`
#![allow(clippy::single_match_else, clippy::match_bool)]
// Some false positives
#![allow(clippy::cargo_common_metadata)]
// Our module organization makes this happen a lot, but struct names should be consistent
#![allow(clippy::module_name_repetitions)]
// We can't super control this, and it shouldn't be a big issue
#![allow(clippy::multiple_crate_versions)]
// `match` can look better than `if` / `else`
#![allow(clippy::single_match)]

// Modules
mod args;
mod images;
mod rect;
mod uvs;

// Exports
pub use rect::Rect;

// Imports
use crate::{images::Images, uvs::ImageUvs};
use anyhow::Context;
use args::Args;
use cgmath::{Matrix4, Point2, Vector2, Vector3};
use glium::{
	glutin::{
		self,
		event::{Event, StartCause, WindowEvent},
		platform::unix::{
			x11::ffi::{PropModeAppend, XA_ATOM},
			EventLoopExtUnix, WindowBuilderExtUnix, WindowExtUnix, XWindowType,
		},
	},
	Surface,
};
use std::{
	mem,
	time::{Duration, Instant},
};
use x11::xlib;

#[allow(clippy::too_many_lines)] // TODO: Refactor
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

	let pos = glutin::dpi::PhysicalPosition {
		x: args.window_geometry.pos[0],
		y: args.window_geometry.pos[1],
	};
	let size = glutin::dpi::PhysicalSize {
		width:  args.window_geometry.size[0],
		height: args.window_geometry.size[1],
	};

	// Create the event loop and build the display.
	let event_loop =
		glium::glutin::event_loop::EventLoop::<!>::new_x11().context("Unable to create an x11 event loop")?;
	let window_builder = glutin::window::WindowBuilder::new()
		.with_position(pos)
		.with_inner_size(size)
		.with_x11_window_type(vec![XWindowType::Desktop]);
	let context_builder = glutin::ContextBuilder::new();
	let display = glium::Display::new(window_builder, context_builder, &event_loop).unwrap();

	// Set the window as always below
	// Note: Required so it doesn't hide itself if the desktop is clicked on
	// TODO: Do this through `glutin`, this is way too hacky
	// SAFETY: TODO
	{
		// Get the xlib display and window
		let gl_window = display.gl_window();
		let window = gl_window.window();
		let display = window.xlib_display().expect("No `X` display found").cast();
		let window = window.xlib_window().expect("No `X` window found");

		// Flush the existing `XMapRaised`
		unsafe { xlib::XFlush(display) };
		std::thread::sleep(Duration::from_millis(100));

		// Unmap the window temporarily
		unsafe { xlib::XUnmapWindow(display, window) };
		unsafe { xlib::XFlush(display) };
		std::thread::sleep(Duration::from_millis(100));

		// Add the always below hint to the window manager
		{
			let property = unsafe { xlib::XInternAtom(display, b"_NET_WM_STATE\0".as_ptr().cast(), 0) };
			let value = unsafe { xlib::XInternAtom(display, b"_NET_WM_STATE_BELOW\0".as_ptr().cast(), 0) };
			let res = unsafe {
				xlib::XChangeProperty(
					display,
					window,
					property,
					XA_ATOM,
					32,
					PropModeAppend,
					(&value as *const u64).cast(),
					1,
				)
			};
			assert_eq!(res, 1, "Unable to change window property");
		}

		// Then remap it
		unsafe { xlib::XMapRaised(display, window) };
		unsafe { xlib::XFlush(display) };
	}

	// Get the window size
	let window_size = display.gl_window().window().inner_size();
	let window_size = [window_size.width, window_size.height];

	// Load images
	let mut images = Images::new(args.images_dir.clone(), args.image_backlog, window_size)
		.with_context(|| format!("Unable to start loading images from {}", args.images_dir.display()))?;

	// Create the indices buffer
	let indices = glium::IndexBuffer::<u32>::new(&display, glium::index::PrimitiveType::TrianglesList, &[
		0, 1, 3, 0, 3, 2,
	])
	.context("Unable to create index buffer")?;

	// Create the program
	let program = {
		glium::Program::new(&display, glium::program::ProgramCreationInput::SourceCode {
			vertex_shader:                  include_str!("vertex.glsl"),
			fragment_shader:                include_str!("frag.glsl"),
			geometry_shader:                None,
			tessellation_control_shader:    None,
			tessellation_evaluation_shader: None,
			transform_feedback_varyings:    None,
			outputs_srgb:                   true,
			uses_point_size:                false,
		})
	}
	.context("Unable to build program")?;

	// All images
	let mut images_data = Vec::new();

	match args.mode {
		args::Mode::Single => {
			let cur_image = Image::new(&display, &mut images, window_size).context("Unable to create image")?;
			let next_image = Image::new(&display, &mut images, window_size).context("Unable to create image")?;
			images_data.push((cur_image, next_image, 0.0, false));
		},
		args::Mode::Grid { width, height } => {
			let [window_width, window_height] = window_size;

			#[allow(clippy::cast_possible_truncation)] // Widths and heights will be small enough for this to not matter
			let cell_size = [window_width / width as u32, window_height / height as u32];

			for _y in 0..height {
				for _x in 0..width {
					let cur_image = Image::new(&display, &mut images, cell_size).context("Unable to create image")?;
					let next_image = Image::new(&display, &mut images, cell_size).context("Unable to create image")?;

					let progress = rand::random();

					images_data.push((cur_image, next_image, progress, true));
				}
			}
		},
	}

	// Run the event loop
	event_loop.run(move |event, _, control_flow| {
		match event {
			Event::WindowEvent { event, .. } => match event {
				// If we got a close request, exit and return
				WindowEvent::CloseRequested | WindowEvent::Destroyed => {
					*control_flow = glutin::event_loop::ControlFlow::Exit;
					return;
				},

				_ => return,
			},
			// If it's time to draw, draw
			Event::NewEvents(StartCause::ResumeTimeReached { .. } | StartCause::Init) => {
				*control_flow =
					glutin::event_loop::ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(1) / 60);
			},
			_ => return,
		}

		// Draw
		let mut target = display.draw();

		// Clear the screen
		target.clear_color(0.0, 0.0, 0.0, 1.0);

		match args.mode {
			args::Mode::Single => {
				let (cur_image, next_image, progress, next_image_is_loaded) = &mut images_data[0];

				self::draw_update(
					&mut target,
					progress,
					&args,
					cur_image,
					next_image,
					&indices,
					&program,
					next_image_is_loaded,
					&display,
					&mut images,
					Vector2::new(1.0, 1.0),
					Point2::new(0.0, 0.0),
				);
			},
			#[allow(clippy::cast_precision_loss)] // Grids will be less than `2^23`
			args::Mode::Grid { width, height } => {
				for y in 0..height {
					for x in 0..width {
						let (cur_image, next_image, progress, next_image_is_loaded) = &mut images_data[width * y + x];

						let scale = Vector2::new(1.0 / (width as f32), 1.0 / (height as f32));
						//let offset = Point2::new((2.0 * x as f32 * scale.x) - 1.0, (2.0 * y as f32 * scale.y) - 1.0);
						//let offset = Point2::new(x as f32 * scale.x, y as f32 * scale.y);
						#[allow(clippy::suboptimal_flops)] // This isn't calculated very often.
						let offset = Point2::new(
							-1.0 + scale.x + 2.0 * scale.x * x as f32,
							-1.0 + scale.y + 2.0 * scale.y * y as f32,
						);

						self::draw_update(
							&mut target,
							progress,
							&args,
							cur_image,
							next_image,
							&indices,
							&program,
							next_image_is_loaded,
							&display,
							&mut images,
							scale,
							offset,
						);
					}
				}
			},
		}

		// Finish drawing
		target.finish().expect("Unable to finish drawing");
	});
}

/// Draws and updates
#[allow(clippy::too_many_arguments)] // TODO: Refactor, closure doesn't work, though
fn draw_update(
	target: &mut glium::Frame, progress: &mut f32, args: &args::Args, cur_image: &mut Image, next_image: &mut Image,
	indices: &glium::IndexBuffer<u32>, program: &glium::Program, next_image_is_loaded: &mut bool,
	facade: &glium::Display, images: &mut Images, scale: Vector2<f32>, offset: Point2<f32>,
) {
	if let Err(err) = self::draw(
		target, *progress, args, cur_image, next_image, indices, program, scale, offset,
	) {
		// Note: We just want to ensure we don't get a panic by dropping an unwrapped target
		let _ = target.set_finish();
		log::warn!("Unable to draw: {err:?}");
	}

	if let Err(err) = self::update(
		progress,
		next_image_is_loaded,
		args,
		cur_image,
		next_image,
		facade,
		images,
	) {
		log::warn!("Unable to update: {err:?}");
	}
}

/// Updates
#[allow(clippy::too_many_arguments)] // It's a binary function, not library
fn update(
	progress: &mut f32, next_image_is_loaded: &mut bool, args: &Args, cur_image: &mut Image, next_image: &mut Image,
	facade: &glium::Display, images: &mut Images,
) -> Result<(), anyhow::Error> {
	// Increase the progress
	*progress += (1.0 / 60.0) / args.duration.as_secs_f32();

	// If the next image isn't loaded, try to load it
	if !*next_image_is_loaded {
		// If our progress is >= fade start, then we have to force wait for the image.
		let force_wait = *progress >= args.fade;

		if force_wait {
			log::warn!("Next image hasn't arrived yet at the end of current image, waiting for it");
		}

		// Then try to load it
		*next_image_is_loaded ^= next_image
			.try_update(facade, images, force_wait)
			.context("Unable to update image")?;

		// If we force waited but the next image isn't loaded, return Err
		if force_wait && !*next_image_is_loaded {
			return Err(anyhow::anyhow!("Unable to load next image even while force-waiting"));
		}
	}

	// If we reached the end, swap the next to current and try to load the next
	if *progress >= 1.0 {
		// Reset the progress to where we where during the fade
		*progress = 1.0 - args.fade;

		// Swap the images
		mem::swap(cur_image, next_image);
		*next_image_is_loaded = false;

		// And try to update the next image
		*next_image_is_loaded ^= next_image
			.try_update(facade, images, false)
			.context("Unable to update image")?;
	}


	Ok(())
}

/// Draws
#[allow(clippy::too_many_arguments)] // TODO: Refactor
fn draw(
	target: &mut glium::Frame, progress: f32, args: &Args, cur_image: &Image, next_image: &Image,
	indices: &glium::IndexBuffer<u32>, program: &glium::Program, scale: Vector2<f32>, offset: Point2<f32>,
) -> Result<(), anyhow::Error> {
	// Calculate the base alpha and progress to apply to the images
	let (base_alpha, next_progress) = match progress {
		f if f >= args.fade => ((progress - args.fade) / (1.0 - args.fade), progress - args.fade),
		_ => (0.0, 0.0),
	};

	// Then draw
	for (image, alpha, progress) in [
		(cur_image, 1.0 - base_alpha, progress),
		(next_image, base_alpha, next_progress),
	] {
		// If alpha is 0, don't render
		if alpha == 0.0 {
			continue;
		}

		let mat = Matrix4::from_translation(Vector3::new(offset.x, offset.y, 0.0)) *
			Matrix4::from_nonuniform_scale(scale.x, scale.y, 1.0);

		let sampler = image.texture.sampled();
		let tex_offset = image.uvs.offset(progress);
		let uniforms = glium::uniform! {
			mat: *<_ as AsRef<[[f32; 4]; 4]>>::as_ref(&mat),
			tex_sampler: sampler,
			tex_offset: tex_offset,
			alpha: alpha,
		};
		let draw_parameters = glium::DrawParameters {
			blend: glium::Blend::alpha_blending(),
			..glium::DrawParameters::default()
		};
		target
			.draw(&image.vertex_buffer, indices, program, &uniforms, &draw_parameters)
			.context("Unable to draw")?;
	}

	Ok(())
}

/// Image
#[derive(Debug)]
struct Image {
	/// Texture
	texture: glium::Texture2d,

	/// Uvs
	uvs: ImageUvs,

	/// Vertex buffer
	vertex_buffer: glium::VertexBuffer<Vertex>,

	/// Window size
	window_size: [u32; 2],
}

impl Image {
	/// Creates a new image
	pub fn new(
		facade: &glium::Display, images: &mut Images, window_size @ [window_width, window_height]: [u32; 2],
	) -> Result<Self, anyhow::Error> {
		let image = images.next_image();

		let image_dims = image.dimensions();
		let texture = glium::texture::Texture2d::new(
			facade,
			glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), image_dims),
		)
		.context("Unable to create texture")?;

		#[allow(clippy::cast_precision_loss)] // Image and window sizes are likely much lower than 2^24
		let uvs = ImageUvs::new(
			image_dims.0 as f32,
			image_dims.1 as f32,
			window_width as f32,
			window_height as f32,
			rand::random(),
		);

		let vertex_buffer = glium::VertexBuffer::dynamic(facade, &Self::vertices(uvs.start()))
			.context("Unable to create vertex buffer")?;
		Ok(Self {
			texture,
			uvs,
			vertex_buffer,
			window_size,
		})
	}

	/// Tries to update this image and returns if actually updated
	pub fn try_update(
		&mut self, facade: &glium::Display, images: &mut Images, force_wait: bool,
	) -> Result<bool, anyhow::Error> {
		let image = match images.try_next_image() {
			Some(image) => image,
			None if force_wait => images.next_image(),
			None => return Ok(false),
		};

		let image_dims = image.dimensions();
		self.texture = glium::texture::Texture2d::new(
			facade,
			glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), image_dims),
		)
		.context("Unable to create texture")?;

		#[allow(clippy::cast_precision_loss)] // Image and window sizes are likely much lower than 2^24
		let uvs = ImageUvs::new(
			image_dims.0 as f32,
			image_dims.1 as f32,
			self.window_size[0] as f32,
			self.window_size[1] as f32,
			rand::random(),
		);
		self.uvs = uvs;

		self.vertex_buffer
			.as_mut_slice()
			.write(&Self::vertices(self.uvs.start()));

		Ok(true)
	}

	/// Creates the vertices for uvs
	const fn vertices(uvs_start: [f32; 2]) -> [Vertex; 4] {
		[
			Vertex {
				vertex_pos: [-1.0, -1.0],
				vertex_tex: [0.0, 0.0],
			},
			Vertex {
				vertex_pos: [1.0, -1.0],
				vertex_tex: [uvs_start[0], 0.0],
			},
			Vertex {
				vertex_pos: [-1.0, 1.0],
				vertex_tex: [0.0, uvs_start[1]],
			},
			Vertex {
				vertex_pos: [1.0, 1.0],
				vertex_tex: uvs_start,
			},
		]
	}
}


/// Vertex
#[derive(Clone, Copy, Debug)]
struct Vertex {
	vertex_pos: [f32; 2],
	vertex_tex: [f32; 2],
}

glium::implement_vertex!(Vertex, vertex_pos, vertex_tex);
