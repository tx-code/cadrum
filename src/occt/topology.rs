use super::ffi;
use crate::{Error, ShapeTopology, TopologyEdge, TopologyEdgeIncident, TopologyEdgeUse, TopologyFace, TopologyFaceUse, TopologyLoop, TopologyOrientation, TopologyShell, TopologyShellRole, Vertex};

const MISSING_INDEX: u32 = u32::MAX;

pub(super) fn snapshot(shape: &ffi::TopoDS_Shape) -> Result<ShapeTopology, Error> {
	let data = ffi::shape_topology(shape);
	if !data.success {
		return Err(Error::TopologyQueryFailed);
	}
	let vertex_count = data.vertices.len();
	let edge_count = data.edges.len();
	let face_count = data.faces.len();

	let vertices = data.vertices.into_iter().map(|vertex| Vertex { runtime_id: vertex.runtime_id, position: crate::DVec3::new(vertex.x, vertex.y, vertex.z), tolerance: vertex.tolerance }).collect();

	let edges = data
		.edges
		.into_iter()
		.map(|edge| {
			Ok(TopologyEdge {
				runtime_id: edge.runtime_id,
				start_vertex: optional_index(edge.start_vertex, vertex_count)?,
				end_vertex: optional_index(edge.end_vertex, vertex_count)?,
				incidents: edge
					.incidents
					.into_iter()
					.map(|incident| {
						Ok(TopologyEdgeIncident {
							face: required_index(incident.face, face_count)?,
							boundary_loop: incident.boundary_loop as usize,
							edge_use: incident.edge_use as usize,
							orientation: orientation(incident.orientation)?,
						})
					})
					.collect::<Result<_, Error>>()?,
			})
		})
		.collect::<Result<Vec<_>, Error>>()?;

	let faces = data
		.faces
		.into_iter()
		.map(|face| {
			Ok(TopologyFace {
				runtime_id: face.runtime_id,
				boundary_loops: face
					.boundary_loops
					.into_iter()
					.map(|boundary_loop| {
						Ok(TopologyLoop {
							is_outer: boundary_loop.is_outer,
							orientation: orientation(boundary_loop.orientation)?,
							edges: boundary_loop.edges.into_iter().map(|edge| Ok(TopologyEdgeUse { edge: required_index(edge.edge, edge_count)?, orientation: orientation(edge.orientation)? })).collect::<Result<_, Error>>()?,
						})
					})
					.collect::<Result<_, Error>>()?,
			})
		})
		.collect::<Result<Vec<_>, Error>>()?;

	for (edge_index, edge) in edges.iter().enumerate() {
		for incident in &edge.incidents {
			let boundary_loop = faces.get(incident.face).and_then(|face| face.boundary_loops.get(incident.boundary_loop)).ok_or(Error::TopologyQueryFailed)?;
			let edge_use = boundary_loop.edges.get(incident.edge_use).ok_or(Error::TopologyQueryFailed)?;
			if edge_use.edge != edge_index || edge_use.orientation != incident.orientation {
				return Err(Error::TopologyQueryFailed);
			}
		}
	}

	let shells = data
		.shells
		.into_iter()
		.map(|shell| {
			Ok(TopologyShell {
				runtime_id: shell.runtime_id,
				role: shell_role(shell.role)?,
				orientation: orientation(shell.orientation)?,
				is_closed: shell.is_closed,
				faces: shell.faces.into_iter().map(|face| Ok(TopologyFaceUse { face: required_index(face.face, face_count)?, orientation: orientation(face.orientation)? })).collect::<Result<_, Error>>()?,
			})
		})
		.collect::<Result<Vec<_>, Error>>()?;

	Ok(ShapeTopology { vertices, edges, faces, shells })
}

fn optional_index(index: u32, upper_bound: usize) -> Result<Option<usize>, Error> {
	(index != MISSING_INDEX).then(|| required_index(index, upper_bound)).transpose()
}

fn required_index(index: u32, upper_bound: usize) -> Result<usize, Error> {
	let index = index as usize;
	(index < upper_bound).then_some(index).ok_or(Error::TopologyQueryFailed)
}

fn shell_role(value: u8) -> Result<TopologyShellRole, Error> {
	match value {
		0 => Ok(TopologyShellRole::Independent),
		1 => Ok(TopologyShellRole::Outer),
		2 => Ok(TopologyShellRole::Cavity),
		_ => Err(Error::TopologyQueryFailed),
	}
}

fn orientation(value: u8) -> Result<TopologyOrientation, Error> {
	match value {
		0 => Ok(TopologyOrientation::Forward),
		1 => Ok(TopologyOrientation::Reversed),
		2 => Ok(TopologyOrientation::Internal),
		3 => Ok(TopologyOrientation::External),
		_ => Err(Error::TopologyQueryFailed),
	}
}
