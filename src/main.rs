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
	drain_filter,
	exclusive_range_pattern
)]
#![warn(unsafe_op_in_unsafe_fn)]

// Modules
mod args;
mod glium_backend;
mod glium_facade;
mod images;
mod uvs;
mod window;

// Imports
use crate::{glium_backend::GliumBackend, glium_facade::GliumFacade, images::Images, uvs::ImageUvs};
use anyhow::Context;
use args::Args;
use glium::Surface;
use std::{mem, rc::Rc};
use window::Window;

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

	// Then create the window
	let window = unsafe { Window::from_window_id(args.window_id) }
		.map(Rc::new)
		.context("Unable to create window")?;

	// Load images
	let images = Images::new(&args.images_dir, &window).context("Unable to load images")?;

	// Create the backend
	let backend = GliumBackend::new(Rc::clone(&window)).context("Unable to create backend")?;

	// And then create the glium facade
	let facade = GliumFacade::new(backend).context("Unable to create glium facade")?;

	// Create the indices buffer
	let indices =
		glium::IndexBuffer::<u32>::new(&facade, glium::index::PrimitiveType::TrianglesList, &[0, 1, 3, 0, 3, 2])
			.context("Unable to create index buffer")?;

	// Create the program
	let program = {
		glium::Program::new(&facade, glium::program::ProgramCreationInput::SourceCode {
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

	// Create both images
	let mut cur_image = Image::new(&facade, &images, &window).context("Unable to create image")?;
	let mut next_image = Image::new(&facade, &images, &window).context("Unable to create image")?;

	let mut progress = 0.0;
	loop {
		// Process events
		window.process_events();

		// Draw
		let mut target = facade.draw();
		let draw_res = self::draw(
			&mut target,
			progress,
			&args,
			&cur_image,
			&next_image,
			&indices,
			&program,
		);

		if let Err(err) = draw_res {
			mem::forget(target);
			log::warn!("Unable to draw: {err:?}");
		}

		// Update
		self::update(
			&mut progress,
			&args,
			&mut cur_image,
			&mut next_image,
			&facade,
			&images,
			&window,
		)?;
	}
}

/// Updates
fn update(
	progress: &mut f32, args: &Args, cur_image: &mut Image, next_image: &mut Image, facade: &GliumFacade,
	images: &Images, window: &Window,
) -> Result<(), anyhow::Error> {
	// Increase the progress
	*progress += (1.0 / 60.0) / args.duration.as_secs_f32();

	// If we reached the end
	if *progress >= 1.0 {
		// Reset the progress to where we where during the fade
		*progress = 1.0 - args.fade;

		// Swap the images
		mem::swap(cur_image, next_image);

		// And update the next
		next_image
			.update(facade, images, window)
			.context("Unable to update image")?;
	}

	Ok(())
}

/// Draws
fn draw(
	target: &mut glium::Frame, progress: f32, args: &Args, cur_image: &Image, next_image: &Image,
	indices: &glium::IndexBuffer<u32>, program: &glium::Program,
) -> Result<(), anyhow::Error> {
	// Calculate the base alpha and progress to apply to the images
	let (base_alpha, next_progress) = match progress {
		f if f >= args.fade => ((progress - args.fade) / (1.0 - args.fade), progress - args.fade),
		_ => (0.0, 0.0),
	};

	// Clear the screen
	target.clear_color(0.0, 0.0, 0.0, 1.0);

	// Then draw
	for (image, alpha, progress) in [
		(cur_image, 1.0 - base_alpha, progress),
		(next_image, base_alpha, next_progress),
	] {
		// If alpha is 0, don't render
		if alpha == 0.0 {
			continue;
		}

		let sampler = image.texture.sampled();
		let tex_offset = image.uvs.offset(progress);
		let uniforms = glium::uniform! {
			tex_sampler: sampler,
			tex_offset: tex_offset,
			alpha: alpha,
		};
		let draw_parameters = glium::DrawParameters {
			blend: glium::Blend::alpha_blending(),
			..Default::default()
		};
		target
			.draw(&image.vertex_buffer, indices, program, &uniforms, &draw_parameters)
			.context("Unable to draw")?;
	}

	// Set the target as finished
	target.set_finish().context("Unable to finish drawing")?;

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
}

impl Image {
	/// Creates a new image
	pub fn new(facade: &GliumFacade, images: &Images, window: &Window) -> Result<Self, anyhow::Error> {
		let image = images.next_image();

		let image_dims = image.dimensions();
		let texture = glium::texture::Texture2d::new(
			facade,
			glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), image_dims),
		)
		.context("Unable to create texture")?;

		let uvs = ImageUvs::new(
			image_dims.0 as f32,
			image_dims.1 as f32,
			window.width() as f32,
			window.height() as f32,
			rand::random(),
		);

		let vertex_buffer = glium::VertexBuffer::dynamic(facade, &Self::vertices(uvs.start()))
			.context("Unable to create vertex buffer")?;
		Ok(Self {
			texture,
			uvs,
			vertex_buffer,
		})
	}

	/// Updates this image
	pub fn update(&mut self, facade: &GliumFacade, images: &Images, window: &Window) -> Result<(), anyhow::Error> {
		let image = images.next_image();

		let image_dims = image.dimensions();
		self.texture = glium::texture::Texture2d::new(
			facade,
			glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), image_dims),
		)
		.context("Unable to create texture")?;

		self.uvs = ImageUvs::new(
			image_dims.0 as f32,
			image_dims.1 as f32,
			window.width() as f32,
			window.height() as f32,
			rand::random(),
		);

		self.vertex_buffer
			.as_mut_slice()
			.write(&Self::vertices(self.uvs.start()));

		Ok(())
	}

	/// Creates the vertices for uvs
	fn vertices(uvs_start: [f32; 2]) -> [Vertex; 4] {
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
