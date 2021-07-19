//! Program

// Imports
use anyhow::Context;
use std::ffi::{CStr, CString};

/// The program
pub struct Program {
	/// Id
	id: u32,

	/// Location of `tex_offset`
	tex_offset_location: i32,
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

		// Get locations
		let tex_location = self::uniform_location(id, "tex").context("Unable to get uniform location")?;
		let tex_offset_location = self::uniform_location(id, "tex_offset").context("Unable to get uniform location")?;

		// Set the tex sampler to texture 0.
		unsafe {
			gl::UseProgram(id);
			gl::Uniform1i(tex_location, 0);
		}

		Ok(Self {
			id,
			tex_offset_location,
		})
	}

	/// Executes code with this program being used
	pub fn with_using<T>(&self, f: impl FnOnce() -> T) -> T {
		// Use this program
		unsafe { gl::UseProgram(self.id) };

		// Execute
		let value = f();

		// Un-use this program
		unsafe { gl::UseProgram(0) };

		value
	}

	/// Returns the tex offset location
	pub fn tex_offset_location(&self) -> i32 {
		self.tex_offset_location
	}
}

/// Returns a uniform location
fn uniform_location(program: u32, name: &str) -> Result<i32, anyhow::Error> {
	// Get the name as a c-string
	let name_cstr = CString::new(name).context("Unable to get name as c-string")?;

	// Then get the location and make sure we found it
	let location = unsafe { gl::GetUniformLocation(program, name_cstr.as_ptr() as *const _) };
	anyhow::ensure!(location > 0, "Location {} not found", name);

	Ok(location)
}
