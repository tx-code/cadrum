use std::io::Read;

use super::{io, shell::Shell, solid::Solid};
use crate::Error;

/// One exact body read from an OCCT BRep payload.
///
/// Shells nested inside a solid remain owned by that solid. Independent open
/// or closed shells are returned separately and are never promoted to solids.
pub enum BrepBody {
	Solid(Solid),
	Shell(Shell),
}

impl BrepBody {
	/// Decode solids and independent shells in one native BRep read.
	pub fn read_brep<R: Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		io::read_brep_bodies(reader)
	}
}
