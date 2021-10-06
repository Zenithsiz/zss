//! Rect

// Imports
use anyhow::Context;
use cgmath::num_traits::Num;
use std::error::Error;

/// A rectangle
pub struct Rect<T> {
	/// Position
	pub pos: [T; 2],

	/// Size
	pub size: [T; 2],
}

impl<T> Rect<T> {
	/// Parses a rect from a geometry, `{width}x{height}+{x}+{y}` or `{width}x{height}`
	#[allow(clippy::shadow_unrelated)] // both `size`s are related
	#[allow(clippy::missing_errors_doc)] // TODO:
	pub fn parse_from_geometry(s: &str) -> Result<Self, anyhow::Error>
	where
		T: Num,
		<T as Num>::FromStrRadixErr: 'static + Send + Sync + Error,
	{
		// Split at the first `+`, or just use it all, if there's no position
		let (size, pos) = s
			.split_once('+')
			.map_or((s, None), |(height, rest)| (height, Some(rest)));

		// Split at the first `x` to get the width and height
		let (width, height) = size.split_once('x').context("Unable to find `x` in size")?;

		let size = [
			T::from_str_radix(width, 10).context("Unable to parse width")?,
			T::from_str_radix(height, 10).context("Unable to parse height")?,
		];

		// Optionally get the position if it exists
		let pos = match pos {
			Some(s) => {
				let (x, y) = s.split_once('+').context("Unable to find `+` in position")?;
				[
					T::from_str_radix(x, 10).context("Unable to parse x")?,
					T::from_str_radix(y, 10).context("Unable to parse y")?,
				]
			},
			None => [T::zero(), T::zero()],
		};

		Ok(Self { pos, size })
	}
}
