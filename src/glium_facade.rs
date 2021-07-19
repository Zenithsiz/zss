//! Glium facade

// Imports
use crate::glium_backend::GliumBackend;
use anyhow::Context as _;
use glium::{
	backend::{Context, Facade},
	debug::DebugCallbackBehavior,
};
use std::rc::Rc;

/// Glium facade
pub struct GliumFacade {
	/// Context
	context: Rc<Context>,
}

impl GliumFacade {
	/// Creates a new display
	pub fn new(backend: GliumBackend) -> Result<Self, anyhow::Error> {
		// SAFETY: The backend has a safe implementation.
		let context = unsafe { Context::new(backend, true, DebugCallbackBehavior::PrintAll) }
			.context("Unable to create context")?;

		Ok(Self { context })
	}

	/// Starts drawing
	pub fn draw(&self) -> glium::Frame {
		glium::Frame::new(Rc::clone(self.get_context()), self.context.get_framebuffer_dimensions())
	}
}

impl Facade for GliumFacade {
	fn get_context(&self) -> &Rc<Context> {
		&self.context
	}
}
