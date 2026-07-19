use super::body::BrepBody;
use super::ffi;
use super::io::{self, ImportedShape};
use super::shell::Shell;
use super::solid::Solid;
use super::stream::RustWriter;
use crate::Error;
use std::io::{Read, Write};

const BODY_ORDER_PREFIX: &[u8] = b"/*CADRUM_BODY_ORDER:";
const BODY_ORDER_SUFFIX: &[u8] = b"*/";
const STEP_FOOTER: &[u8] = b"END-ISO-10303-21;";

#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyKind {
	Solid,
	Shell,
}

struct BodyCompound {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	order: Vec<BodyKind>,
	#[cfg(feature = "color")]
	colormap: std::collections::HashMap<u64, crate::Color>,
}

impl BodyCompound {
	fn empty() -> Self {
		Self {
			inner: ffi::make_empty(),
			order: Vec::new(),
			#[cfg(feature = "color")]
			colormap: std::collections::HashMap::new(),
		}
	}

	fn add_solid(&mut self, solid: &Solid) {
		ffi::compound_add(self.inner.pin_mut(), solid.inner());
		self.order.push(BodyKind::Solid);
		#[cfg(feature = "color")]
		self.colormap.extend(solid.colormap().iter().map(|(&id, &color)| (id, color)));
	}

	fn add_shell(&mut self, shell: &Shell) {
		ffi::compound_add(self.inner.pin_mut(), shell.inner());
		self.order.push(BodyKind::Shell);
	}

	fn from_bodies<'a>(bodies: impl IntoIterator<Item = &'a BrepBody>) -> Self {
		let mut compound = Self::empty();
		for body in bodies {
			match body {
				BrepBody::Solid(solid) => compound.add_solid(solid),
				BrepBody::Shell(shell) => compound.add_shell(shell),
			}
		}
		compound
	}

	fn from_shells<'a>(shells: impl IntoIterator<Item = &'a Shell>) -> Self {
		let mut compound = Self::empty();
		for shell in shells {
			compound.add_shell(shell);
		}
		compound
	}
}

pub(super) fn read_step_bodies<R: Read>(reader: &mut R) -> Result<Vec<BrepBody>, Error> {
	let mut payload = Vec::new();
	reader.read_to_end(&mut payload).map_err(|_| Error::StepReadFailed)?;
	let order = read_body_order(&payload);
	let bodies = decompose(io::read_step_shape(&mut payload.as_slice(), true)?, Error::StepReadFailed)?;
	if let Some(order) = order {
		if can_apply_body_order(&bodies, &order) {
			return apply_body_order(bodies, &order).ok_or(Error::StepReadFailed);
		}
	}
	Ok(bodies)
}

pub(super) fn read_brep_bodies<R: Read>(reader: &mut R) -> Result<Vec<BrepBody>, Error> {
	decompose(io::read_brep_shape(reader)?, Error::BrepReadFailed)
}

pub(super) fn read_step_shells<R: Read>(reader: &mut R) -> Result<Vec<Shell>, Error> {
	let shells = read_step_bodies(reader)?
		.into_iter()
		.filter_map(|body| match body {
			BrepBody::Shell(shell) => Some(shell),
			BrepBody::Solid(_) => None,
		})
		.collect::<Vec<_>>();
	(!shells.is_empty()).then_some(shells).ok_or(Error::StepReadFailed)
}

fn decompose(imported: ImportedShape, empty_error: Error) -> Result<Vec<BrepBody>, Error> {
	let bodies = ffi::decompose_into_brep_bodies(&imported.inner);
	let result = bodies
		.iter()
		.filter_map(|shape| {
			if ffi::shape_is_solid(shape) {
				Some(BrepBody::Solid(Solid::new(
					ffi::clone_shape_handle(shape),
					#[cfg(feature = "color")]
					imported.colormap.clone(),
					Default::default(),
				)))
			} else if ffi::shape_is_shell(shape) {
				Some(BrepBody::Shell(Shell::new(ffi::clone_shape_handle(shape))))
			} else {
				None
			}
		})
		.collect::<Vec<_>>();
	(!result.is_empty()).then_some(result).ok_or(empty_error)
}

fn read_body_order(payload: &[u8]) -> Option<Vec<BodyKind>> {
	let start = payload.windows(BODY_ORDER_PREFIX.len()).rposition(|window| window == BODY_ORDER_PREFIX)? + BODY_ORDER_PREFIX.len();
	let end = payload[start..].windows(BODY_ORDER_SUFFIX.len()).position(|window| window == BODY_ORDER_SUFFIX)? + start;
	payload[start..end]
		.iter()
		.map(|kind| match kind {
			b'D' => Some(BodyKind::Solid),
			b'H' => Some(BodyKind::Shell),
			_ => None,
		})
		.collect()
}

fn can_apply_body_order(bodies: &[BrepBody], order: &[BodyKind]) -> bool {
	bodies.len() == order.len() && bodies.iter().filter(|body| matches!(body, BrepBody::Solid(_))).count() == order.iter().filter(|&&kind| kind == BodyKind::Solid).count() && bodies.iter().filter(|body| matches!(body, BrepBody::Shell(_))).count() == order.iter().filter(|&&kind| kind == BodyKind::Shell).count()
}

fn apply_body_order(bodies: Vec<BrepBody>, order: &[BodyKind]) -> Option<Vec<BrepBody>> {
	let mut solids = std::collections::VecDeque::new();
	let mut shells = std::collections::VecDeque::new();
	for body in bodies {
		match body {
			BrepBody::Solid(solid) => solids.push_back(BrepBody::Solid(solid)),
			BrepBody::Shell(shell) => shells.push_back(BrepBody::Shell(shell)),
		}
	}
	order
		.iter()
		.map(|kind| match kind {
			BodyKind::Solid => solids.pop_front(),
			BodyKind::Shell => shells.pop_front(),
		})
		.collect()
}

pub(super) fn write_step_bodies<'a, W: Write>(bodies: impl IntoIterator<Item = &'a BrepBody>, writer: &mut W) -> Result<(), Error> {
	write_step(BodyCompound::from_bodies(bodies), writer)
}

pub(super) fn write_step_shells<'a, W: Write>(shells: impl IntoIterator<Item = &'a Shell>, writer: &mut W) -> Result<(), Error> {
	write_step(BodyCompound::from_shells(shells), writer)
}

fn write_step<W: Write>(compound: BodyCompound, writer: &mut W) -> Result<(), Error> {
	if compound.order.is_empty() {
		return Err(Error::StepWriteFailed);
	}
	let _guard = io::lock_step_io();
	let mut payload = Vec::new();
	#[cfg(feature = "color")]
	{
		let mut ids = Vec::with_capacity(compound.colormap.len());
		let mut rgb = Vec::with_capacity(compound.colormap.len() * 3);
		for (&id, color) in &compound.colormap {
			ids.push(id);
			rgb.extend_from_slice(&[color.r, color.g, color.b]);
		}
		let mut rust_writer = RustWriter::from_ref(&mut payload);
		if !ffi::write_step_color_stream(&compound.inner, &ids, &rgb, &mut rust_writer) {
			return Err(Error::StepWriteFailed);
		}
	}
	#[cfg(not(feature = "color"))]
	{
		let mut rust_writer = RustWriter::from_ref(&mut payload);
		if !ffi::write_step_stream(&compound.inner, &mut rust_writer) {
			return Err(Error::StepWriteFailed);
		}
	}
	drop(_guard);
	insert_body_order(&mut payload, &compound.order)?;
	writer.write_all(&payload).map_err(|_| Error::StepWriteFailed)
}

fn insert_body_order(payload: &mut Vec<u8>, order: &[BodyKind]) -> Result<(), Error> {
	let footer = payload.windows(STEP_FOOTER.len()).rposition(|window| window == STEP_FOOTER).ok_or(Error::StepWriteFailed)?;
	let mut marker = Vec::with_capacity(BODY_ORDER_PREFIX.len() + order.len() + BODY_ORDER_SUFFIX.len() + 2);
	marker.extend_from_slice(BODY_ORDER_PREFIX);
	marker.extend(order.iter().map(|kind| match kind {
		BodyKind::Solid => b'D',
		BodyKind::Shell => b'H',
	}));
	marker.extend_from_slice(BODY_ORDER_SUFFIX);
	marker.extend_from_slice(b"\r\n");
	payload.splice(footer..footer, marker);
	Ok(())
}

pub(super) fn write_brep_bodies<'a, W: Write>(bodies: impl IntoIterator<Item = &'a BrepBody>, writer: &mut W) -> Result<(), Error> {
	let compound = BodyCompound::from_bodies(bodies);
	if compound.order.is_empty() {
		return Err(Error::BrepWriteFailed);
	}
	{
		let mut rust_writer = RustWriter::from_ref(writer);
		if !ffi::write_brep_stream(&compound.inner, &mut rust_writer) {
			return Err(Error::BrepWriteFailed);
		}
	}
	#[cfg(feature = "color")]
	io::write_color_trailer(&compound.inner, &compound.colormap, writer)?;
	Ok(())
}
