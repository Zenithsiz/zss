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
	pub fn from_window_id(id: u64) -> Result<Self, anyhow::Error> {
		// Get the display and screen
		// TODO: Window might not be from the default display, somehow obtain
		//       the correct display eventually. Maybe same with screen?
		// SAFETY: These functions should be inherently safe to use, they take
		//         no arguments (aside from `NULL`, which is valid), so no UB
		//         should be possible.
		let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
		let screen = unsafe { xlib::XDefaultScreen(display) };

		// Get the window attributes
		// SAFETY: Even if `id` isn't a valid window, this should simply return `0`,
		//         which we catch before the `assume_init` call.
		let mut attrs = MaybeUninit::uninit();
		anyhow::ensure!(
			unsafe { xlib::XGetWindowAttributes(display, id, attrs.as_mut_ptr()) } != 0,
			"Unable to get window attributes"
		);
		let attrs = unsafe { attrs.assume_init() };

		// Get the frame-buffer configs
		// SAFETY: We terminate the `FRAME_BUFFER_CONFIG_ATTRIBUTES` and aside
		//         from that, the function should be inherently safe.
		let mut fb_configs_len = MaybeUninit::uninit();
		let fb_configs = unsafe {
			glx::glXChooseFBConfig(
				display,
				screen,
				Self::FRAME_BUFFER_CONFIG_ATTRIBUTES.as_ptr(),
				fb_configs_len.as_mut_ptr(),
			)
		};
		anyhow::ensure!(!fb_configs.is_null(), "Unable to retrieve any valid fb configs");

		// SAFETY: By here, we know the previous call succeeded and thus the variable
		//         is initialized.
		let fb_configs_len = unsafe { fb_configs_len.assume_init() };
		log::info!("Found {fb_configs_len} frame-buffer configurations at {fb_configs:?}");
		anyhow::ensure!(fb_configs_len != 0, "No fg configs found");

		// Then select the first one we find
		// TODO: Maybe pick one based on something?
		// SAFETY: We just checked there's at least 1 config here.
		let fb_config = unsafe { *fb_configs };

		// Get the function to create the gl context
		// SAFETY: The call to the function is safe, as we null terminate the string,
		//         and the cast is also safe, as that's the signature of the returned function.
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
		// SAFETY: We null-terminate `GL_CONFIG_ATTRIBUTES`,
		//         every other argument has no possible UB and
		//         the function should be inherently safe.
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
		// SAFETY: Checking for events and receiving them should be safe.
		while unsafe { xlib::XPending(self.display) } != 0 {
			let mut event = MaybeUninit::uninit();
			unsafe { xlib::XNextEvent(self.display, event.as_mut_ptr()) };
		}
	}

	/// Returns if the gl context is current
	pub fn is_context_current(&self) -> bool {
		// SAFETY: No arguments are involved, call should be inherently safe.
		let gl_context = unsafe { glx::glXGetCurrentContext() };
		gl_context == self.gl_context
	}

	/// Makes the current gl context current
	pub fn make_context_current(&self) -> Result<(), anyhow::Error> {
		// SAFETY: The display, window id and gl context are known to be valid, thus
		//         the call should be safe.
		let res = unsafe { glx::glXMakeContextCurrent(self.display, self.id, self.id, self.gl_context) };

		anyhow::ensure!(res == 1, "Failed to make context current");
		Ok(())
	}

	/// Swaps buffers
	pub fn swap_buffers(&self) {
		// SAFETY: display and the window id are known to be valid, thus
		//         the cal should be safe
		unsafe {
			glx::glXSwapBuffers(self.display, self.id);
		}
	}
}
