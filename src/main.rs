//! Zss

// Features
#![feature(
	raw_ref_op,
	format_args_capture,
	atomic_mut_ptr,
	bindings_after_at,
	destructuring_assignment
)]

// Modules
pub mod x;

// Imports
use anyhow::Context;
use std::{
	ffi::{CStr, CString},
	mem,
};

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
	let mut window_state = x::XWindowState::new(window).context("Unable to initialize open-gl context")?;

	// Compile the shaders into a program
	let program = create_program()?;

	// Create the textures
	let cur_tex;
	let next_tex;
	let cur_image;
	unsafe {
		let mut texs = [0; 2];
		gl::GenTextures(2, texs.as_mut_ptr());
		[cur_tex, next_tex] = texs;

		gl::BindTexture(gl::TEXTURE_2D, cur_tex);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

		cur_image = image::open("/home/filipe/.wallpaper/active/34.png")
			.context("Unable to open image")?
			.flipv()
			.to_rgba8();

		gl::TexImage2D(
			gl::TEXTURE_2D,
			0,
			gl::RGBA as i32,
			cur_image.width() as i32,
			cur_image.height() as i32,
			0,
			gl::RGBA,
			gl::UNSIGNED_BYTE,
			cur_image.as_ptr() as *const _,
		);
		gl::GenerateMipmap(gl::TEXTURE_2D);

		gl::UseProgram(program);
		gl::Uniform1i(gl::GetUniformLocation(program, b"cur_tex\0".as_ptr() as *const i8), 0);
		gl::Uniform1i(gl::GetUniformLocation(program, b"next_tex\0".as_ptr() as *const i8), 1);
	}

	let cur_image_ar = cur_image.height() as f32 / cur_image.width() as f32;

	#[rustfmt::skip]
	let vertices: [f32; 16] = [
		// Vertex  /   Uvs
		-1.0, -2.0 * cur_image_ar,  0.0, 0.0,
		 1.0, -2.0 * cur_image_ar,  1.0, 0.0,
		-1.0,  2.0 * cur_image_ar,  0.0, 1.0,
		 1.0,  2.0 * cur_image_ar,  1.0, 1.0,
	];

	let indices = [0, 1, 3, 0, 2, 3];

	// Create the vao and vertex buffers
	let mut vao = 0;
	let vertex_buffer;
	let index_buffer;
	unsafe {
		gl::GenVertexArrays(1, &mut vao);
		let mut buffers = [0; 2];
		gl::GenBuffers(2, buffers.as_mut_ptr());
		[vertex_buffer, index_buffer] = buffers;

		gl::BindVertexArray(vao);
		gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer);
		gl::BufferData(
			gl::ARRAY_BUFFER,
			(mem::size_of::<f32>() * vertices.len()) as isize,
			vertices.as_ptr() as *const _,
			gl::STATIC_DRAW,
		);

		gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_buffer);
		gl::BufferData(
			gl::ELEMENT_ARRAY_BUFFER,
			(mem::size_of::<i32>() * indices.len()) as isize,
			indices.as_ptr() as *const _,
			gl::STATIC_DRAW,
		);

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

	// Main Loop
	let mut progress: f32 = 0.0;
	loop {
		// Check for events
		window_state.process_events();

		// Then draw
		unsafe {
			gl::ClearColor(0.0, 0.0, 0.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

			gl::ActiveTexture(gl::TEXTURE0);
			gl::BindTexture(gl::TEXTURE_2D, cur_tex);
			gl::ActiveTexture(gl::TEXTURE1);
			gl::BindTexture(gl::TEXTURE_2D, next_tex);

			gl::UseProgram(program);
			gl::Uniform1f(
				gl::GetUniformLocation(program, b"progress\0".as_ptr() as *const _),
				progress,
			);

			gl::BindVertexArray(vao);
			gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null_mut());
			gl::BindVertexArray(0);

			if progress >= 2.0 * cur_image_ar {
				progress = -2.0 * cur_image_ar;
			} else {
				progress += cur_image_ar / 60.0 / 10.0;
			}

			window_state.swap_buffers();
		}
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

	Ok(program)
}
