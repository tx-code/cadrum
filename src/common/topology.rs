use glam::DVec3;

/// Orientation of one topological occurrence inside its immediate owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopologyOrientation {
	Forward,
	Reversed,
	Internal,
	External,
}

/// Immutable value snapshot of one exact BRep vertex.
///
/// `runtime_id` identifies the current OCCT `TShape` only while the imported or
/// constructed model remains alive. Persist the vector index or an application
/// ID derived by the caller instead of serializing this value.
#[derive(Debug, Clone, PartialEq)]
pub struct Vertex {
	pub runtime_id: u64,
	pub position: DVec3,
	pub tolerance: f64,
}

/// One use of an edge by an ordered face boundary loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopologyEdgeUse {
	pub edge: usize,
	pub orientation: TopologyOrientation,
}

/// One edge occurrence viewed from the incident face side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopologyEdgeIncident {
	pub face: usize,
	pub boundary_loop: usize,
	pub edge_use: usize,
	pub orientation: TopologyOrientation,
}

/// One unique exact BRep edge and every face-side occurrence that uses it.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyEdge {
	pub runtime_id: u64,
	pub start_vertex: Option<usize>,
	pub end_vertex: Option<usize>,
	pub incidents: Vec<TopologyEdgeIncident>,
}

impl TopologyEdge {
	/// An edge with more than two face-side uses is non-manifold.
	///
	/// A seam normally has two uses by the same face and is therefore manifold.
	pub fn is_non_manifold(&self) -> bool {
		self.incidents.len() > 2
	}

	/// A single face-side use denotes an exposed boundary edge.
	pub fn is_boundary(&self) -> bool {
		self.incidents.len() == 1
	}
}

/// One ordered wire on a face.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyLoop {
	pub is_outer: bool,
	pub orientation: TopologyOrientation,
	pub edges: Vec<TopologyEdgeUse>,
}

/// One unique exact BRep face with its outer and inner wires in native order.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyFace {
	pub runtime_id: u64,
	pub boundary_loops: Vec<TopologyLoop>,
}

/// One face occurrence inside a shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopologyFaceUse {
	pub face: usize,
	pub orientation: TopologyOrientation,
}

/// Role of a shell in the queried body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopologyShellRole {
	Independent,
	Outer,
	Cavity,
}

/// One shell occurrence inside a body.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyShell {
	pub runtime_id: u64,
	pub role: TopologyShellRole,
	pub orientation: TopologyOrientation,
	pub is_closed: bool,
	pub faces: Vec<TopologyFaceUse>,
}

/// Ordered, index-addressed topology snapshot of one exact Solid or Shell.
///
/// Indices are valid only inside this snapshot. They deliberately avoid
/// exposing OCCT pointer identity as a persistent exchange contract.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeTopology {
	pub vertices: Vec<Vertex>,
	pub edges: Vec<TopologyEdge>,
	pub faces: Vec<TopologyFace>,
	pub shells: Vec<TopologyShell>,
}
