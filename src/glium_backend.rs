//! Glium backend

// Imports
use crate::window::Window;
use std::{ffi::CString, rc::Rc};
use x11::glx;

/// Glium backend
pub struct GliumBackend {
	/// Window
	window: Rc<Window>,
}

impl GliumBackend {
	pub fn new(window: Rc<Window>) -> Result<Self, anyhow::Error> {
		Ok(Self { window })
	}
}

// SAFETY: The implementation of each function is safe
unsafe impl glium::backend::Backend for GliumBackend {
	fn swap_buffers(&self) -> Result<(), glium::SwapBuffersError> {
		self.window.swap_buffers();
		Ok(())
	}

	unsafe fn get_proc_address(&self, name: &str) -> *const std::ffi::c_void {
		let name_cstr = CString::new(name).expect("Unable to create c-string from name");
		// SAFETY: `glXGetProcAddressARB` should be safe to call with any string.
		match unsafe { glx::glXGetProcAddressARB(name_cstr.as_ptr() as *const u8) } {
			Some(f) => f as *const _,
			None => {
				log::warn!("Unable to load {name}");
				std::ptr::null()
			},
		}
	}

	fn get_framebuffer_dimensions(&self) -> (u32, u32) {
		(self.window.width(), self.window.height())
	}

	fn is_current(&self) -> bool {
		self.window.is_context_current()
	}

	unsafe fn make_current(&self) {
		self.window
			.make_context_current()
			.expect("Unable to make context current")
	}
}
