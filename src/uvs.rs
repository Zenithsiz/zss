//! Image uvs

pub struct Uvs {
	/// uvs
	start: [f32; 2],

	/// End
	end: [f32; 2],

	/// Swap direction
	swap_dir: bool,
}

impl Uvs {
	/// Creates the uvs for an image
	pub fn new(image_width: f32, image_height: f32, window_width: f32, window_height: f32, swap_dir: bool) -> Self {
		let (start, end) = match image_width / image_height >= window_width / window_height {
			true => ([(window_width / image_width) / (window_height / image_height), 1.0], [
				1.0, 1.0,
			]),
			false => ([1.0, (window_height / image_height) / (window_width / image_width)], [
				1.0, 1.0,
			]),
		};

		Self { start, end, swap_dir }
	}

	/// Returns the starting uvs
	pub fn start(&self) -> [f32; 2] {
		self.start
	}

	/// Returns the offset given progress
	pub fn offset(&self, f: f32) -> [f32; 2] {
		let f = match self.swap_dir {
			true => 1.0 - f,
			false => f,
		};

		[f * (self.end[0] - self.start[0]), f * (self.end[1] - self.start[1])]
	}
}
