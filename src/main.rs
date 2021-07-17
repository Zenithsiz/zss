#![feature(raw_ref_op, format_args_capture, atomic_mut_ptr)]

use std::{
	ffi::{CStr, CString},
	mem::{self, MaybeUninit},
	os::raw::c_int,
	sync::atomic::{self, AtomicI32},
};

use anyhow::Context;
use x11::{glx, xlib};

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

	// Get the display and screen
	let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
	let screen = unsafe { xlib::XDefaultScreen(display) };

	// Get the window attributes
	let mut window_attrs: xlib::XWindowAttributes = unsafe { MaybeUninit::zeroed().assume_init() };
	unsafe { xlib::XGetWindowAttributes(display, window, &mut window_attrs) };

	// Get the frame-buffer configs
	#[rustfmt::skip]
	let fb_config_attributes = [
		glx::GLX_RENDER_TYPE  , glx::GLX_RGBA_BIT,
		glx::GLX_DRAWABLE_TYPE, glx::GLX_PBUFFER_BIT,
		glx::GLX_DOUBLEBUFFER , xlib::True,
		glx::GLX_RED_SIZE     , 8,
		glx::GLX_GREEN_SIZE   , 8,
		glx::GLX_BLUE_SIZE    , 8,
		glx::GLX_ALPHA_SIZE   , 8,
		glx::GLX_DEPTH_SIZE   , 16,
		glx::GLX_NONE,
	];
	let fb_configs_len = AtomicI32::new(0);
	let fb_configs = unsafe {
		glx::glXChooseFBConfig(
			display,
			screen,
			&fb_config_attributes as *const i32,
			fb_configs_len.as_mut_ptr(),
		)
	};
	let fb_configs_len = fb_configs_len.load(atomic::Ordering::Acquire);
	log::info!("Found {fb_configs_len} frame-buffer configurations");

	// Then select the first one we find
	// TODO: Maybe pick one based on something?
	anyhow::ensure!(!fb_configs.is_null() && fb_configs_len != 0, "No fg configs found");
	let fb_config = unsafe { *fb_configs };

	// Create the gl context and make it current
	let create_gl_context = unsafe { glx::glXGetProcAddressARB(b"glXCreateContextAttribsARB\0" as *const _) }
		.context("Unable to get function")?;
	let create_gl_context: unsafe fn(
		*mut xlib::Display,
		glx::GLXFBConfig,
		glx::GLXContext,
		xlib::Bool,
		*const c_int,
	) -> glx::GLXContext = unsafe { mem::transmute(create_gl_context) };

	#[rustfmt::skip]
	let gl_attrs = [
		0x2091, 3,
		0x2092, 0,
		0x2094, 0x2,
		0x9126, 0x1,
		0, 0
	];
	let gl_context =
		unsafe { create_gl_context(display, fb_config, std::ptr::null_mut(), xlib::True, gl_attrs.as_ptr()) };

	//let gl_context =
	//	unsafe { glx::glXCreateNewContext(display, fb_config, glx::GLX_RGBA_TYPE, std::ptr::null_mut(), xlib::True) };
	anyhow::ensure!(!gl_context.is_null(), "Unable to get gl context");
	unsafe {
		log::info!("Making context {gl_context:?} current");
		anyhow::ensure!(
			glx::glXMakeContextCurrent(display, window, window, gl_context) == 1,
			"Failed to make context current"
		);
	}

	// Load all gl functions
	unsafe {
		gl::load_with(|name| {
			let name_cstr = CString::new(name).expect("Unable to create c-string from name");
			match glx::glXGetProcAddressARB(name_cstr.as_ptr() as *const u8) {
				Some(f) => f as *const _,
				None => {
					log::warn!("Unable to load {name}");
					std::ptr::null()
				},
			}
		})
	};

	// Log some info about the gl implementation
	let gl_version = unsafe { gl::GetString(gl::VERSION) };
	let gl_version = unsafe { CStr::from_ptr(gl_version as *const _) };
	log::info!("Gl version: {gl_version:?}");

	// Setup gl
	unsafe {
		gl::DrawBuffer(gl::BACK);
		gl::Viewport(0, 0, window_attrs.width, window_attrs.height);
	}

	// Compile the shaders into a program
	let program;
	unsafe {
		let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
		let frag_shader = gl::CreateShader(gl::FRAGMENT_SHADER);

		let vertex_src =
			CString::new(*include_bytes!("vertex.glsl")).context("Unable to get vertex shader a c-string")?;
		let frag_src = CString::new(*include_bytes!("frag.glsl")).context("Unable to get frag shader a c-string")?;

		gl::ShaderSource(vertex_shader, 1, &vertex_src.as_ptr(), std::ptr::null());
		gl::ShaderSource(frag_shader, 1, &frag_src.as_ptr(), std::ptr::null());

		gl::CompileShader(vertex_shader);
		gl::CompileShader(frag_shader);

		{
			let mut success = 0;
			gl::GetShaderiv(vertex_shader, gl::COMPILE_STATUS, &mut success);
			if success == 0 {
				let mut info = [0; 1024];
				let mut info_len = 0;
				gl::GetShaderInfoLog(vertex_shader, 1024, &mut info_len, info.as_mut_ptr() as *mut i8);
				let info = CStr::from_bytes_with_nul(&info[..(info_len as usize + 1)])
					.context("Unable to get info as c-string")?;
				return Err(anyhow::anyhow!("Unable to compile vertex shader: {:?}", info));
			}
		}
		{
			let mut success = 0;
			gl::GetShaderiv(frag_shader, gl::COMPILE_STATUS, &mut success);
			if success == 0 {
				let mut info = [0; 1024];
				let mut info_len = 0;
				gl::GetShaderInfoLog(frag_shader, 1024, &mut info_len, info.as_mut_ptr() as *mut i8);
				let info = CStr::from_bytes_with_nul(&info[..(info_len as usize + 1)])
					.context("Unable to get info as c-string")?;
				return Err(anyhow::anyhow!("Unable to compile vertex shader: {:?}", info));
			}
		}

		program = gl::CreateProgram();
		gl::AttachShader(program, vertex_shader);
		gl::AttachShader(program, frag_shader);
		gl::LinkProgram(program);

		gl::DeleteShader(vertex_shader);
		gl::DeleteShader(frag_shader);
	}

	#[rustfmt::skip]
	let vertices = [
		-0.5, -0.5,
		 0.5, -0.5,
		 0.0,  0.5,
	];

	// Create the vao and vertex buffers
	let mut vao = 0;
	let mut vertex_buffer = 0;
	unsafe {
		gl::GenVertexArrays(1, &mut vao);
		gl::GenBuffers(1, &mut vertex_buffer);

		gl::BindVertexArray(vao);
		gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer);
		gl::BufferData(
			gl::ARRAY_BUFFER,
			(mem::size_of::<f32>() * vertices.len()) as isize,
			vertices.as_ptr() as *const _,
			gl::STATIC_DRAW,
		);

		gl::VertexAttribPointer(
			0,
			2,
			gl::FLOAT,
			gl::FALSE,
			2 * mem::size_of::<f32>() as i32,
			std::ptr::null(),
		);
		gl::EnableVertexAttribArray(0);

		gl::BindBuffer(gl::ARRAY_BUFFER, 0);
		gl::BindVertexArray(0);
	}

	// Main Loop
	let _f: f32 = 0.0;
	loop {
		// Check for events
		while unsafe { xlib::XPending(display) } != 0 {
			let mut event = xlib::XEvent { type_: 0 };
			unsafe { xlib::XNextEvent(display, &mut event) };

			log::warn!("Received event {event:?}");
		}

		// Then draw
		unsafe {
			gl::ClearColor(1.0, 0.0, 0.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

			gl::UseProgram(program);
			gl::BindVertexArray(vao);
			gl::DrawArrays(gl::TRIANGLES, 0, 3);

			//f += 0.01;

			glx::glXSwapBuffers(display, window);
		}
	}
}
