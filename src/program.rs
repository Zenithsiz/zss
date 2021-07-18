//! Program

// Imports
use anyhow::Context;
use std::ffi::{CStr, CString};

/// The program
pub struct Program {
	/// Id
	id: u32,
}

impl Program {
	// Vertex source
	const FRAG_SRC: &'static str = include_str!("frag.glsl");
	// Vertex source
	const VERTEX_SRC: &'static str = include_str!("vertex.glsl");

	/// Creates a new program
	pub fn new() -> Result<Self, anyhow::Error> {
		// Load the sources for both shaders
		let vertex_src = CString::new(Self::VERTEX_SRC).context("Unable to get vertex shader a c-string")?;
		let frag_src = CString::new(Self::FRAG_SRC).context("Unable to get frag shader a c-string")?;

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
		let id = unsafe { gl::CreateProgram() };
		unsafe {
			gl::AttachShader(id, vertex_shader);
			gl::AttachShader(id, frag_shader);
			gl::LinkProgram(id);
		}

		{
			let mut success = 0;
			unsafe {
				gl::GetProgramiv(id, gl::LINK_STATUS, &mut success);
			}
			if success == 0 {
				let mut info = [0; 1024];
				let mut info_len = 0;
				unsafe {
					gl::GetProgramInfoLog(id, 1024, &mut info_len, info.as_mut_ptr() as *mut i8);
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

		// Set the tex sampler to texture 0.
		unsafe {
			gl::UseProgram(id);
			gl::Uniform1i(gl::GetUniformLocation(id, b"tex\0".as_ptr() as *const i8), 0);
		}

		Ok(Self { id })
	}

	/// Uses this program
	// TODO: Make this RAII
	pub fn use_program(&self) {
		unsafe {
			gl::UseProgram(self.id);
		}
	}

	/// Returns a uniform location
	pub fn uniform_location(&self, name: &str) -> Result<i32, anyhow::Error> {
		// Get the name as a c-string
		let name_cstr = CString::new(name).context("Unable to get name as c-string")?;

		// Then get the location and make sure we found it
		let location = unsafe { gl::GetUniformLocation(self.id, name_cstr.as_ptr() as *const _) };
		anyhow::ensure!(location > 0, "Location {} not found", name);

		Ok(location)
	}
}
