//! Zss

// Features
#![feature(
	raw_ref_op,
	format_args_capture,
	atomic_mut_ptr,
	bindings_after_at,
	destructuring_assignment
)]
#![warn(unsafe_op_in_unsafe_fn)]

// Modules
mod texture;
mod window;

// Imports
use anyhow::Context;
use image::{GenericImageView, ImageBuffer, Rgba};
use rand::prelude::SliceRandom;
use std::{
	ffi::{CStr, CString},
	mem,
	path::Path,
};
use texture::Texture;

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
	let program = self::create_program()?;

	// Create the vao
	let indices = [0, 1, 3, 0, 2, 3];
	let (vertex_buffer, vao) = self::create_vao(&indices);

	// Create the tex
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
		window_width,
		window_height,
		vertex_buffer,
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

			gl::ActiveTexture(gl::TEXTURE0);
			tex.bind();

			gl::UseProgram(program);
			gl::Uniform2f(
				gl::GetUniformLocation(program, b"tex_offset\0".as_ptr() as *const _),
				tex_offset[0],
				tex_offset[1],
			);

			gl::BindVertexArray(vao);
			gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null_mut());
			gl::BindVertexArray(0);

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
					window_width,
					window_height,
					vertex_buffer,
					rand::random(),
				)?;
				cur_path += 1;
			}

			window_state.swap_buffers();
		}
	}
}

/// Opens and setups a new image
#[allow(clippy::type_complexity)] // TODO
fn setup_new_image(
	path: impl AsRef<Path>, window_width: u32, window_height: u32, vertex_buffer: u32, swap_dir: bool,
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
	self::update_tex(&image);

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
	self::update_vertices(vertex_buffer, &vertices);
	Ok((dir, tex_offset, max))
}

/// Updates a texture
fn update_tex(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) {
	unsafe {
		gl::TexImage2D(
			gl::TEXTURE_2D,
			0,
			gl::RGBA as i32,
			image.width() as i32,
			image.height() as i32,
			0,
			gl::RGBA,
			gl::UNSIGNED_BYTE,
			image.as_ptr() as *const _,
		);
		gl::GenerateMipmap(gl::TEXTURE_2D);
	}
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

/// Creates the vao with the buffers
fn create_vao(indices: &[i32]) -> (u32, u32) {
	let mut vao = 0;
	let vertex_buffer;
	let index_buffer;
	unsafe {
		gl::GenVertexArrays(1, &mut vao);
		let mut buffers = [0; 2];
		gl::GenBuffers(2, buffers.as_mut_ptr());
		[vertex_buffer, index_buffer] = buffers;

		gl::BindVertexArray(vao);
		gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_buffer);
		gl::BufferData(
			gl::ELEMENT_ARRAY_BUFFER,
			(mem::size_of::<i32>() * indices.len()) as isize,
			indices.as_ptr() as *const _,
			gl::STATIC_DRAW,
		);

		gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer);
		gl::VertexAttribPointer(
			0,
			2,
			gl::FLOAT,
			gl::FALSE,
			4 * mem::size_of::<f32>() as i32,
			std::ptr::null(),
		);
		gl::EnableVertexAttribArray(0);
		gl::VertexAttribPointer(
			1,
			2,
			gl::FLOAT,
			gl::FALSE,
			4 * mem::size_of::<f32>() as i32,
			std::ptr::null::<f32>().wrapping_add(2) as *const _,
		);
		gl::EnableVertexAttribArray(1);


		gl::BindBuffer(gl::ARRAY_BUFFER, 0);
		gl::BindVertexArray(0);
	}
	(vertex_buffer, vao)
}

/// Updates the vertices
fn update_vertices(vertex_buffer: u32, vertices: &[f32]) {
	unsafe {
		gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer);
		gl::BufferData(
			gl::ARRAY_BUFFER,
			(mem::size_of::<f32>() * vertices.len()) as isize,
			vertices.as_ptr() as *const _,
			gl::STATIC_DRAW,
		);
	}
}

/// Creates the open-gl program
fn create_program() -> Result<u32, anyhow::Error> {
	// Load the sources for both shaders
	let vertex_src = CString::new(*include_bytes!("vertex.glsl")).context("Unable to get vertex shader a c-string")?;
	let frag_src = CString::new(*include_bytes!("frag.glsl")).context("Unable to get frag shader a c-string")?;


	// Create the two shaders
	let vertex_shader = unsafe { gl::CreateShader(gl::VERTEX_SHADER) };
	let frag_shader = unsafe { gl::CreateShader(gl::FRAGMENT_SHADER) };

	// Then compile them
	unsafe {
		gl::ShaderSource(vertex_shader, 1, &vertex_src.as_ptr(), std::ptr::null());
		gl::ShaderSource(frag_shader, 1, &frag_src.as_ptr(), std::ptr::null());

		gl::CompileShader(vertex_shader);
		gl::CompileShader(frag_shader);
	}

	// Check for any errors on either
	{
		let mut success = 0;
		unsafe {
			gl::GetShaderiv(vertex_shader, gl::COMPILE_STATUS, &mut success);
		}
		if success == 0 {
			let mut info = [0; 1024];
			let mut info_len = 0;
			unsafe {
				gl::GetShaderInfoLog(vertex_shader, 1024, &mut info_len, info.as_mut_ptr() as *mut i8);
			}
			let info = CStr::from_bytes_with_nul(&info[..(info_len as usize + 1)])
				.context("Unable to get info as c-string")?;
			return Err(anyhow::anyhow!("Unable to compile vertex shader: {:?}", info));
		}
	}
	{
		let mut success = 0;
		unsafe {
			gl::GetShaderiv(frag_shader, gl::COMPILE_STATUS, &mut success);
		}
		if success == 0 {
			let mut info = [0; 1024];
			let mut info_len = 0;
			unsafe {
				gl::GetShaderInfoLog(frag_shader, 1024, &mut info_len, info.as_mut_ptr() as *mut i8);
			}
			let info = CStr::from_bytes_with_nul(&info[..(info_len as usize + 1)])
				.context("Unable to get info as c-string")?;
			return Err(anyhow::anyhow!("Unable to compile vertex shader: {:?}", info));
		}
	}

	// Finally create the program, attach both shaders and link it
	let program = unsafe { gl::CreateProgram() };
	unsafe {
		gl::AttachShader(program, vertex_shader);
		gl::AttachShader(program, frag_shader);
		gl::LinkProgram(program);
	}

	// TODO: Linking errors?
	{
		let mut success = 0;
		unsafe {
			gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);
		}
		if success == 0 {
			let mut info = [0; 1024];
			let mut info_len = 0;
			unsafe {
				gl::GetProgramInfoLog(program, 1024, &mut info_len, info.as_mut_ptr() as *mut i8);
			}
			let info = CStr::from_bytes_with_nul(&info[..(info_len as usize + 1)])
				.context("Unable to get info as c-string")?;
			return Err(anyhow::anyhow!("Unable to link program: {:?}", info));
		}
	}

	// Finally delete the shaders
	unsafe {
		gl::DeleteShader(vertex_shader);
		gl::DeleteShader(frag_shader);
	}

	// Set the tex sampler to texture 0.
	unsafe {
		gl::UseProgram(program);
		gl::Uniform1i(gl::GetUniformLocation(program, b"tex\0".as_ptr() as *const i8), 0);
	}

	Ok(program)
}
