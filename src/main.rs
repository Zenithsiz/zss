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
use std::rc::Rc;
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

	// Load all images
	let images = Images::new(&args.images_dir, &window).context("Unable to load images")?;

	// Create the backend
	let backend = GliumBackend::new(Rc::clone(&window)).context("Unable to create backend")?;

	// And then create the glium facade
	let facade = GliumFacade::new(backend).context("Unable to create glium facade")?;

	let mut image = images.next_image();
	let mut uvs = ImageUvs::new(
		image.width() as f32,
		image.height() as f32,
		window.width() as f32,
		window.height() as f32,
		false,
	);

	let vertices = [
		Vertex {
			vertex_pos: [-1.0, -1.0],
			vertex_tex: [0.0, 0.0],
		},
		Vertex {
			vertex_pos: [1.0, -1.0],
			vertex_tex: [uvs.start()[0], 0.0],
		},
		Vertex {
			vertex_pos: [-1.0, 1.0],
			vertex_tex: [0.0, uvs.start()[1]],
		},
		Vertex {
			vertex_pos: [1.0, 1.0],
			vertex_tex: uvs.start(),
		},
	];

	let mut vertex_buffer = glium::VertexBuffer::new(&facade, &vertices).context("Unable to create vertex buffer")?;

	let indices =
		glium::IndexBuffer::<u32>::new(&facade, glium::index::PrimitiveType::TrianglesList, &[0, 1, 3, 0, 3, 2])
			.context("Unable to create index buffer")?;

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


	let mut texture = {
		let image_dims = image.dimensions();
		glium::texture::Texture2d::new(
			&facade,
			glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), image_dims),
		)
		.context("Unable to create texture")?
	};

	let mut progress = 0.0;
	loop {
		window.process_events();

		let mut target = facade.draw();
		target.clear_color(0.0, 0.0, 1.0, 1.0);

		let sampler = texture.sampled();

		let tex_offset = uvs.offset(progress);
		let uniforms = glium::uniform! {
			tex_sampler: sampler,
			tex_offset: tex_offset,
		};

		target
			.draw(&vertex_buffer, &indices, &program, &uniforms, &Default::default())
			.context("Unable to draw")?;

		target.finish().context("Unable to finish drawing")?;


		progress += (1.0 / 60.0) / args.duration.as_secs_f32();

		if progress >= 1.0 {
			progress = 0.0;

			image = images.next_image();

			let image_dims = image.dimensions();
			texture = glium::texture::Texture2d::new(
				&facade,
				glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), image_dims),
			)
			.context("Unable to create texture")?;

			uvs = ImageUvs::new(
				image_dims.0 as f32,
				image_dims.1 as f32,
				window.width() as f32,
				window.height() as f32,
				rand::random(),
			);

			let vertices = [
				Vertex {
					vertex_pos: [-1.0, -1.0],
					vertex_tex: [0.0, 0.0],
				},
				Vertex {
					vertex_pos: [1.0, -1.0],
					vertex_tex: [uvs.start()[0], 0.0],
				},
				Vertex {
					vertex_pos: [-1.0, 1.0],
					vertex_tex: [0.0, uvs.start()[1]],
				},
				Vertex {
					vertex_pos: [1.0, 1.0],
					vertex_tex: uvs.start(),
				},
			];

			vertex_buffer.as_mut_slice().write(&vertices);
		}
	}
}

#[derive(Copy, Clone)]
struct Vertex {
	vertex_pos: [f32; 2],
	vertex_tex: [f32; 2],
}

glium::implement_vertex!(Vertex, vertex_pos, vertex_tex);
