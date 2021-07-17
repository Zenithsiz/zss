#![feature(
	raw_ref_op,
	format_args_capture,
	atomic_mut_ptr,
	bindings_after_at,
	destructuring_assignment
)]

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

	// Enable gl errors
	unsafe {
		gl::Enable(gl::DEBUG_OUTPUT);
		gl::DebugMessageCallback(Some(gl_debug_callback), std::ptr::null());
	}

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
		while unsafe { xlib::XPending(display) } != 0 {
			let mut event = xlib::XEvent { type_: 0 };
			unsafe { xlib::XNextEvent(display, &mut event) };

			log::warn!("Received event {event:?}");
		}

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

			glx::glXSwapBuffers(display, window);
		}
	}
}

extern "system" fn gl_debug_callback(
	source: u32, kind: u32, id: u32, severity: u32, length: i32, msg: *const i8, _: *mut std::ffi::c_void,
) {
	let msg = match length {
		// If negative, `msg` is null-terminated
		length if length < 0 => unsafe { CStr::from_ptr(msg).to_string_lossy() },
		_ => {
			let slice = unsafe { std::slice::from_raw_parts(msg as *const u8, length as usize) };
			String::from_utf8_lossy(slice)
		},
	};

	let source = match source {
		gl::DEBUG_SOURCE_API => "Api",
		gl::DEBUG_SOURCE_APPLICATION => "Application",
		gl::DEBUG_SOURCE_OTHER => "Other",
		gl::DEBUG_SOURCE_SHADER_COMPILER => "Shader Compiler",
		gl::DEBUG_SOURCE_THIRD_PARTY => "Third Party",
		gl::DEBUG_SOURCE_WINDOW_SYSTEM => "Window System",
		_ => "<Unknown>",
	};

	// TODO: Do something about `PUSH/POP_GROUP`?
	let kind = match kind {
		gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "Deprecated Behavior",
		gl::DEBUG_TYPE_ERROR => "Error",
		gl::DEBUG_TYPE_MARKER => "Marker",
		gl::DEBUG_TYPE_OTHER => "Other",
		gl::DEBUG_TYPE_PERFORMANCE => "Performance",
		gl::DEBUG_TYPE_POP_GROUP => "Pop Group",
		gl::DEBUG_TYPE_PORTABILITY => "Portability",
		gl::DEBUG_TYPE_PUSH_GROUP => "Push Group",
		gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "Undefined Behavior",
		_ => "<Unknown>",
	};

	let log_level = match severity {
		gl::DEBUG_SEVERITY_HIGH => log::Level::Error,
		gl::DEBUG_SEVERITY_LOW => log::Level::Info,
		gl::DEBUG_SEVERITY_MEDIUM => log::Level::Warn,
		gl::DEBUG_SEVERITY_NOTIFICATION => log::Level::Debug,
		_ => log::Level::Trace,
	};

	log::log!(log_level, "[{source}]:[{kind}]:{id}: {msg}");
}
