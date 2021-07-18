//! Texture

// Imports
use std::mem::MaybeUninit;

use image::{ImageBuffer, Rgba};

/// A texture
pub struct Texture {
	/// Id
	id: u32,
}

impl Texture {
	/// Creates a new texture
	#[allow(clippy::new_without_default)] // It does non-trivial global initialization
	pub fn new() -> Self {
		// Generate the texture
		let mut id = MaybeUninit::uninit();
		unsafe {
			gl::GenTextures(1, id.as_mut_ptr());
		}
		let id = unsafe { id.assume_init() };

		// Then set it's wrap and min/mag filters
		unsafe {
			gl::BindTexture(gl::TEXTURE_2D, id);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
		}

		Self { id }
	}

	/// Binds this texture
	pub fn bind(&self) {
		unsafe {
			gl::BindTexture(gl::TEXTURE_2D, self.id);
		}
	}

	/// Updates this texture
	pub fn update(&self, image: &ImageBuffer<Rgba<u8>, Vec<u8>>) {
		// Bind ourselves
		self.bind();

		// Then upload and generate mip-maps
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
}
