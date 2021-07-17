#![feature(raw_ref_op, format_args_capture)]

use std::{ffi::CString, mem::MaybeUninit};

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
	let mut fb_configs_len = 0;
	let fb_configs = unsafe {
		glx::glXChooseFBConfig(
			display,
			screen,
			&fb_config_attributes as *const i32,
			&mut fb_configs_len,
		)
	};
	log::info!("Found {fb_configs_len} frame-buffer configurations");

	// Then select the first one we find
	// TODO: Maybe pick one based on something?
	anyhow::ensure!(!fb_configs.is_null() && fb_configs_len != 0, "No fg configs found");
	let fb_config = unsafe { *fb_configs };

	// Create the gl context and make it current
	let gl_attrs = [glx::GLX_NONE];
	let gl_window = unsafe { glx::glXCreateWindow(display, fb_config, window, &gl_attrs as *const i32) };
	let gl_context =
		unsafe { glx::glXCreateNewContext(display, fb_config, glx::GLX_RGBA_TYPE, std::ptr::null_mut(), xlib::True) };
	anyhow::ensure!(!gl_context.is_null(), "Unable to get gl context");
	unsafe {
		log::info!("Making context {gl_context:?} current");
		anyhow::ensure!(
			glx::glXMakeContextCurrent(display, gl_window, gl_window, gl_context) == 1,
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

	// Setup gl
	unsafe {
		gl::DrawBuffer(gl::BACK);
		gl::Viewport(0, 0, window_attrs.width, window_attrs.height);
	}

	// Main Loop
	let mut f: f32 = 0.0;
	loop {
		// Check for events
		while unsafe { xlib::XPending(display) } != 0 {
			let mut event = xlib::XEvent { type_: 0 };
			unsafe { xlib::XNextEvent(display, &mut event) };

			log::warn!("Received event {event:?}");
		}

		// Then draw
		unsafe {
			gl::ClearColor(f.sin().abs(), (0.23562 * f).sin().abs(), (1.35672 * f).sin().abs(), 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

			f += 0.01;

			glx::glXSwapBuffers(display, gl_window);
		}
	}
}
