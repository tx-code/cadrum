/// Why a shell could not be promoted to a solid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolidificationFailure {
	EmptyShellSet,
	InvalidShell,
	OpenShell { boundary_edge_count: usize },
	NonManifoldShell { edge_count: usize },
	InvalidConstituentShell { shell_index: usize },
	OpenConstituentShell { shell_index: usize, boundary_edge_count: usize },
	NonManifoldConstituentShell { shell_index: usize, edge_count: usize },
	CavityNotContained { shell_index: usize },
	ShellIntersection { first_shell_index: usize, second_shell_index: usize },
	BuildFailed,
	OrientationFailed,
	InvalidSolid,
	NonPositiveVolume,
	KernelFailure,
}

impl std::fmt::Display for SolidificationFailure {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::EmptyShellSet => write!(f, "at least one outer shell is required"),
			Self::InvalidShell => write!(f, "shell topology is invalid"),
			Self::OpenShell { boundary_edge_count } => write!(f, "shell is open ({boundary_edge_count} boundary edges)"),
			Self::NonManifoldShell { edge_count } => write!(f, "shell is non-manifold ({edge_count} edges have more than two incident faces)"),
			Self::InvalidConstituentShell { shell_index } => write!(f, "shell {shell_index} topology is invalid"),
			Self::OpenConstituentShell { shell_index, boundary_edge_count } => write!(f, "shell {shell_index} is open ({boundary_edge_count} boundary edges)"),
			Self::NonManifoldConstituentShell { shell_index, edge_count } => write!(f, "shell {shell_index} is non-manifold ({edge_count} edges have more than two incident faces)"),
			Self::CavityNotContained { shell_index } => write!(f, "cavity shell {shell_index} is not strictly contained by the outer shell"),
			Self::ShellIntersection { first_shell_index, second_shell_index } => write!(f, "shells {first_shell_index} and {second_shell_index} touch or intersect"),
			Self::BuildFailed => write!(f, "OCCT could not build a solid from the shell"),
			Self::OrientationFailed => write!(f, "closed solid orientation could not be resolved"),
			Self::InvalidSolid => write!(f, "constructed solid topology is invalid"),
			Self::NonPositiveVolume => write!(f, "constructed solid does not have finite positive volume"),
			Self::KernelFailure => write!(f, "OCCT failed while validating the solid"),
		}
	}
}

/// Errors that can occur during CAD operations.
#[derive(Debug)]
pub enum Error {
	/// STEP file read failed (invalid format or corrupted data).
	StepReadFailed,

	/// BRep file read failed (invalid format or corrupted data).
	BrepReadFailed,

	/// STEP file write failed.
	StepWriteFailed,

	/// BRep file write failed.
	BrepWriteFailed,

	/// Triangulation/meshing failed.
	TriangulationFailed,

	/// Exact ordered topology could not be queried without returning partial data.
	TopologyQueryFailed,

	/// Boolean operation (fuse/cut/common) failed.
	BooleanOperationFailed,

	/// 単一 Solid を期待する演算 (`+`/`-`/`*` 演算子) で結果の Solid 数が 1 でなかった。
	/// `usize` は実際の結果 Solid 数 (0 = 非交差/全削除、2+ = 結果が複数ピースに分割)。
	/// 戻り値が複数ピースになりうる場合は `Solid::boolean_union/subtract/intersect` を
	/// 直接使い `Vec<Solid>` を受け取ること。
	OneFailed(usize),

	/// Shape cleaning (UnifySameDomain) failed.
	CleanFailed,

	/// Helix edge construction failed (e.g. degenerate parameters).
	HelixFailed,

	/// Extrusion (`Solid::extrude`) failed: empty profile, zero-length
	/// direction, or profile not closed.
	ExtrudeFailed,

	/// Pipe sweep (`Solid::sweep`) failed: profile not closed, edges not
	/// connectable into a wire, or `BRepOffsetAPI_MakePipe` returned no shape.
	SweepFailed,

	/// Shell / hollow (`Solid::shell` via `BRepOffsetAPI_MakeThickSolid`)
	/// failed: thickness sign incompatible with geometry, sharp corners
	/// yielding a self-intersecting offset surface, or OCCT internal failure.
	ShellFailed,

	/// Fillet (`Solid::fillet_edges` via `BRepFilletAPI_MakeFillet`) failed:
	/// radius too large for the local geometry, tangent discontinuity along
	/// the selected edge chain, or an edge not belonging to `self` was passed.
	FilletFailed,

	/// Chamfer (`Solid::chamfer_edges` via `BRepFilletAPI_MakeChamfer`) failed:
	/// distance too large for the local geometry, tangent discontinuity along
	/// the selected edge chain, or an edge not belonging to `self` was passed.
	ChamferFailed,

	/// Lofting (`Solid::loft` / `BRepOffsetAPI_ThruSections`) failed: section
	/// count too low, section wire ill-formed, or OCCT internal failure.
	/// The string identifies which precondition or stage failed.
	LoftFailed(String),

	/// Sewing (`Shell::sew` or `Solid::sew`) failed because the faces do not
	/// form the required single connected shell within the given tolerance.
	SewFailed(String),

	/// A shell failed explicit closed-body validation and was not promoted.
	SolidificationFailed(SolidificationFailure),

	/// Surface offset (`Solid::offset_surface` / `BRepOffsetAPI_MakeOffsetShape`)
	/// failed: the offset surfaces self-intersect (thin walls/slots thinner
	/// than 2x the offset magnitude) or OCCT rejected the join. The string
	/// carries the offending parameters.
	OffsetFailed(String),

	/// B-spline solid (`Solid::bspline`) construction failed: grid too small,
	/// surface interpolation rejected the input, or sewing/capping failed.
	/// The string identifies which stage failed and with what parameters.
	BsplineFailed(String),

	/// Exact trimmed B-spline face construction or extraction failed.
	TrimmedFaceFailed(String),

	/// Edge construction failed due to degenerate input (e.g. collinear arc
	/// points, zero-length line, negative radius). The string describes which
	/// constructor failed and with which parameters.
	InvalidEdge(String),

	/// SVG export (HLR projection) failed.
	SvgExportFailed,

	/// PNG export failed (rasterizer / encoder / writer).
	PngExportFailed,

	/// STL write failed.
	StlWriteFailed,

	/// glTF (GLB binary) write failed.
	GltfWriteFailed,

	/// Invalid color string (unrecognized name or invalid hex format).
	InvalidColor(String),

	/// Unknown error.
	Unknown(String),
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::StepReadFailed => write!(f, "STEP read failed"),
			Error::BrepReadFailed => write!(f, "BRep read failed"),
			Error::StepWriteFailed => write!(f, "STEP write failed"),
			Error::BrepWriteFailed => write!(f, "BRep write failed"),
			Error::TriangulationFailed => write!(f, "Triangulation failed"),
			Error::TopologyQueryFailed => write!(f, "Exact topology query failed"),
			Error::BooleanOperationFailed => write!(f, "Boolean operation failed"),
			Error::OneFailed(n) => write!(f, "Expected exactly one resulting Solid, got {}", n),
			Error::CleanFailed => write!(f, "Shape clean failed"),
			Error::HelixFailed => write!(f, "Helix failed"),
			Error::ExtrudeFailed => write!(f, "Extrude failed"),
			Error::SweepFailed => write!(f, "Sweep failed"),
			Error::ShellFailed => write!(f, "Shell failed"),
			Error::FilletFailed => write!(f, "Fillet failed"),
			Error::ChamferFailed => write!(f, "Chamfer failed"),
			Error::LoftFailed(msg) => write!(f, "Loft failed: {}", msg),
			Error::SewFailed(msg) => write!(f, "Sew failed: {}", msg),
			Error::SolidificationFailed(reason) => write!(f, "Solidification failed: {reason}"),
			Error::OffsetFailed(msg) => write!(f, "Offset failed: {}", msg),
			Error::BsplineFailed(msg) => write!(f, "Bspline failed: {}", msg),
			Error::TrimmedFaceFailed(msg) => write!(f, "Trimmed face failed: {}", msg),
			Error::InvalidEdge(msg) => write!(f, "Invalid edge: {}", msg),
			Error::SvgExportFailed => write!(f, "SVG export failed"),
			Error::PngExportFailed => write!(f, "PNG export failed"),
			Error::StlWriteFailed => write!(f, "STL write failed"),
			Error::GltfWriteFailed => write!(f, "glTF write failed"),
			Error::InvalidColor(s) => write!(f, "Invalid color: \"{}\"", s),
			Error::Unknown(msg) => write!(f, "Unknown error: {}", msg),
		}
	}
}

impl std::error::Error for Error {}
