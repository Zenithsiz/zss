//! Window

// Imports
use anyhow::Context;
use std::{
	mem::{self, MaybeUninit},
	os::raw::c_int,
};
use x11::{glx, xlib};

/// Window
pub struct Window {
	/// Display
	display: *mut xlib::Display,

	/// Id
	id: u64,

	/// Gl context
	gl_context: glx::GLXContext,

	/// Attributes
	attrs: xlib::XWindowAttributes,
}

impl Window {
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

	/// Creates a window from an existing x11 window
	///
	/// # Safety
	/// `window_id` must be a valid X window.
	pub unsafe fn from_window_id(id: u64) -> Result<Self, anyhow::Error> {
		// Get the display and screen
		// TODO: Window might not be from the default display, somehow obtain
		//       the correct display eventually. Maybe same with screen?
		let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
		let screen = unsafe { xlib::XDefaultScreen(display) };

		// Get the window attributes
		let mut attrs: xlib::XWindowAttributes = unsafe { MaybeUninit::zeroed().assume_init() };
		unsafe { xlib::XGetWindowAttributes(display, id, &mut attrs) };

		// Get the frame-buffer configs
		// TODO: Check if there's UB here, atomic solved the issue, but might still exist.
		let mut fb_configs_len = MaybeUninit::uninit();
		let fb_configs = unsafe {
			glx::glXChooseFBConfig(
				display,
				screen,
				Self::FRAME_BUFFER_CONFIG_ATTRIBUTES.as_ptr(),
				fb_configs_len.as_mut_ptr(),
			)
		};
		let fb_configs_len = unsafe { fb_configs_len.assume_init() };
		log::info!("Found {fb_configs_len} frame-buffer configurations at {fb_configs:?}");
		anyhow::ensure!(!fb_configs.is_null() && fb_configs_len != 0, "No fg configs found");

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

		Ok(Self {
			display,
			gl_context,
			id,
			attrs,
		})
	}

	/// Window size
	pub fn size(&self) -> [u32; 2] {
		[self.width(), self.height()]
	}

	/// Window width
	pub fn width(&self) -> u32 {
		self.attrs.width as u32
	}

	/// Window height
	pub fn height(&self) -> u32 {
		self.attrs.height as u32
	}

	/// Processes all X events
	pub fn process_events(&self) {
		while unsafe { xlib::XPending(self.display) } != 0 {
			let mut event = xlib::XEvent { type_: 0 };
			unsafe { xlib::XNextEvent(self.display, &mut event) };

			log::warn!("Received event {event:?}");
		}
	}

	/// Returns if the gl context is current
	pub fn is_context_current(&self) -> bool {
		let gl_context = unsafe { glx::glXGetCurrentContext() };
		gl_context == self.gl_context
	}

	/// Makes the current gl context current
	pub fn make_context_current(&self) -> Result<(), anyhow::Error> {
		let res = unsafe { glx::glXMakeContextCurrent(self.display, self.id, self.id, self.gl_context) };

		anyhow::ensure!(res == 1, "Failed to make context current");
		Ok(())
	}

	/// Swaps buffers
	pub fn swap_buffers(&self) {
		unsafe {
			glx::glXSwapBuffers(self.display, self.id);
		}
	}
}
