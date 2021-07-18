//! X initialization

// Imports
use anyhow::Context;
use std::{
	ffi::{CStr, CString},
	mem::{self, MaybeUninit},
	os::raw::c_int,
	sync::atomic::{self, AtomicI32},
};
use x11::{glx, xlib};

/// X Window state
pub struct XWindowState {
	/// Display
	display: *mut xlib::Display,

	/// window
	window: u64,
}

impl XWindowState {
	/// Frame buffer configuration attributes
	#[rustfmt::skip]
	const FRAME_BUFFER_CONFIG_ATTRIBUTES: [i32; 17] = [
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
	/// Open-gl configuration attributes
	#[rustfmt::skip]
	const GL_CONFIG_ATTRIBUTES: [i32; 10] = [
		0x2091, 3,
		0x2092, 0,
		0x2094, 0x2,
		0x9126, 0x1,
		0, 0
	];

	/// Creates a new window state from an existing window
	pub fn new(window: u64) -> Result<Self, anyhow::Error> {
		// Get the display and screen
		// TODO: Window might not be from the default display, somehow obtain
		//       the correct display eventually. Maybe same with screen?
		let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
		let screen = unsafe { xlib::XDefaultScreen(display) };

		// Get the window attributes
		let mut window_attrs: xlib::XWindowAttributes = unsafe { MaybeUninit::zeroed().assume_init() };
		unsafe { xlib::XGetWindowAttributes(display, window, &mut window_attrs) };

		// Get the frame-buffer configs
		// TODO: Check if there's UB here, atomic solved the issue, but might still exist.
		let fb_configs_len = AtomicI32::new(0);
		let fb_configs = unsafe {
			glx::glXChooseFBConfig(
				display,
				screen,
				Self::FRAME_BUFFER_CONFIG_ATTRIBUTES.as_ptr(),
				fb_configs_len.as_mut_ptr(),
			)
		};
		let fb_configs_len = fb_configs_len.load(atomic::Ordering::Acquire);
		anyhow::ensure!(!fb_configs.is_null() && fb_configs_len != 0, "No fg configs found");
		log::info!("Found {fb_configs_len} frame-buffer configurations");

		// Then select the first one we find
		// TODO: Maybe pick one based on something?
		let fb_config = unsafe { *fb_configs };

		// Get the function to create the gl context
		let create_gl_context = unsafe { glx::glXGetProcAddressARB(b"glXCreateContextAttribsARB\0" as *const _) }
			.context("Unable to get function")?;
		let create_gl_context: unsafe fn(
			*mut xlib::Display,
			glx::GLXFBConfig,
			glx::GLXContext,
			xlib::Bool,
			*const c_int,
		) -> glx::GLXContext = unsafe { mem::transmute(create_gl_context) };

		// Then create the context
		let gl_context = unsafe {
			create_gl_context(
				display,
				fb_config,
				std::ptr::null_mut(),
				xlib::True,
				Self::GL_CONFIG_ATTRIBUTES.as_ptr(),
			)
		};
		anyhow::ensure!(!gl_context.is_null(), "Unable to get gl context");

		// And make it current
		unsafe {
			log::info!("Making context {gl_context:?} current");
			anyhow::ensure!(
				glx::glXMakeContextCurrent(display, window, window, gl_context) == 1,
				"Failed to make context current"
			);
		}

		// Finally load all gl functions
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

		// And log info about which gl version we got.
		let gl_version = unsafe { gl::GetString(gl::VERSION) };
		let gl_version = unsafe { CStr::from_ptr(gl_version as *const _) };
		log::info!("Gl version: {gl_version:?}");

		// Enable gl errors
		unsafe {
			gl::Enable(gl::DEBUG_OUTPUT);
			gl::DebugMessageCallback(Some(gl_debug_callback), std::ptr::null());
		}

		// Setup the buffer and viewport from the window
		unsafe {
			gl::DrawBuffer(gl::BACK);
			gl::Viewport(0, 0, window_attrs.width, window_attrs.height);
		}

		Ok(Self { display, window })
	}

	/// Processes all X events
	pub fn process_events(&mut self) {
		while unsafe { xlib::XPending(self.display) } != 0 {
			let mut event = xlib::XEvent { type_: 0 };
			unsafe { xlib::XNextEvent(self.display, &mut event) };

			log::warn!("Received event {event:?}");
		}
	}

	/// Swaps buffers
	pub fn swap_buffers(&mut self) {
		unsafe {
			glx::glXSwapBuffers(self.display, self.window);
		}
	}
}

/// Debug callback for gl.
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
