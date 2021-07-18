//! Vao

use std::mem::{self, MaybeUninit};

/// Vao
pub struct Vao {
	/// Id
	id: u32,

	/// Vertex buffer id
	vertex_buffer_id: u32,
}

impl Vao {
	/// Indices for this vao
	const INDICES: &'static [u32] = &[0, 1, 3, 0, 3, 2];

	/// Creates a new vao
	pub fn new() -> Self {
		// Generate the vao
		let mut id = MaybeUninit::uninit();
		unsafe {
			gl::GenVertexArrays(1, id.as_mut_ptr());
		}
		let id = unsafe { id.assume_init() };

		// Generate the buffers
		let mut buffers = MaybeUninit::uninit_array();
		unsafe {
			gl::GenBuffers(2, buffers.as_mut_ptr().cast());
		}
		let [vertex_buffer_id, index_buffer_id] = unsafe { MaybeUninit::array_assume_init(buffers) };

		// Upload the indices buffer
		unsafe {
			gl::BindVertexArray(id);
			gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_buffer_id);
			gl::BufferData(
				gl::ELEMENT_ARRAY_BUFFER,
				mem::size_of_val(Self::INDICES) as isize,
				Self::INDICES.as_ptr() as *const _,
				gl::STATIC_DRAW,
			);
		}

		// Then set the vertex attributes for the vertex buffer
		unsafe {
			gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer_id);
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
		}

		// Finally unbind ourselves
		unsafe {
			gl::BindBuffer(gl::ARRAY_BUFFER, 0);
			gl::BindVertexArray(0);
		}

		Self { id, vertex_buffer_id }
	}

	/// Executes code with this vao bound
	pub fn with_bound<T>(&self, f: impl FnOnce() -> T) -> T {
		// Bind ourselves and the vertex buffer
		unsafe {
			gl::BindVertexArray(self.id);
			gl::BindBuffer(gl::ARRAY_BUFFER, self.vertex_buffer_id);
		}


		// Execute
		let value = f();

		// Unbind ourselves and the vertex buffer
		unsafe {
			gl::BindVertexArray(0);
			gl::BindBuffer(gl::ARRAY_BUFFER, 0);
		}

		value
	}

	/// Updates the vertices
	pub fn update_vertices(&self, vertices: &[f32]) {
		self.with_bound(|| unsafe {
			gl::BufferData(
				gl::ARRAY_BUFFER,
				mem::size_of_val(vertices) as isize,
				vertices.as_ptr() as *const _,
				gl::STATIC_DRAW,
			);
		})
	}
}
