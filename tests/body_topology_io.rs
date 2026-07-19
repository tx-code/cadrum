use cadrum::{BrepBody, DVec3, Error, Shell, Solid, SolidificationFailure, TopologyOrientation, TopologyShellRole};

fn closed_shell(min: DVec3, max: DVec3) -> Shell {
	let source = Solid::cube(min, max);
	Shell::sew(source.iter_face(), 1.0e-7).expect("closed cube shell")
}

fn open_shell(min: DVec3, max: DVec3) -> Shell {
	let source = Solid::cube(min, max);
	Shell::sew(source.iter_face().take(5), 1.0e-7).expect("open cube shell")
}

fn body_kinds(bodies: &[BrepBody]) -> Vec<&'static str> {
	bodies
		.iter()
		.map(|body| match body {
			BrepBody::Solid(_) => "solid",
			BrepBody::Shell(_) => "shell",
		})
		.collect()
}

fn body_min_x(body: &BrepBody) -> f64 {
	body.topology().expect("body topology").vertices.iter().map(|vertex| vertex.position.x).fold(f64::INFINITY, f64::min)
}

fn solidification_error(result: Result<Solid, Error>, message: &str) -> Error {
	match result {
		Ok(_) => panic!("{message}"),
		Err(error) => error,
	}
}

#[test]
fn cube_topology_is_ordered_and_two_sided() {
	let topology = Solid::cube(DVec3::ZERO, DVec3::ONE).topology().expect("cube topology");

	assert_eq!(topology.vertices.len(), 8);
	assert_eq!(topology.edges.len(), 12);
	assert_eq!(topology.faces.len(), 6);
	assert_eq!(topology.shells.len(), 1);
	assert!(topology.shells[0].is_closed);
	assert_eq!(topology.shells[0].role, TopologyShellRole::Outer);
	assert_eq!(topology.shells[0].faces.len(), 6);
	assert!(topology.vertices.iter().all(|vertex| vertex.position.is_finite() && vertex.tolerance.is_finite() && vertex.tolerance >= 0.0));
	assert!(topology.edges.iter().all(|edge| edge.start_vertex.is_some() && edge.end_vertex.is_some() && edge.start_vertex != edge.end_vertex && edge.incidents.len() == 2 && !edge.is_boundary() && !edge.is_non_manifold()));
	assert!(topology.faces.iter().all(|face| { face.boundary_loops.len() == 1 && face.boundary_loops[0].is_outer && face.boundary_loops[0].edges.len() == 4 }));
}

#[test]
fn open_shell_topology_exposes_boundary_incidence() {
	let shell = open_shell(DVec3::ZERO, DVec3::ONE);
	let topology = shell.topology().expect("open shell topology");

	assert_eq!(topology.shells.len(), 1);
	assert!(!topology.shells[0].is_closed);
	assert_eq!(topology.shells[0].role, TopologyShellRole::Independent);
	assert_eq!(topology.faces.len(), 5);
	assert_eq!(topology.edges.iter().filter(|edge| edge.is_boundary()).count(), 4);
	assert!(topology.edges.iter().all(|edge| !edge.is_non_manifold()));
}

#[test]
fn periodic_face_seam_has_two_uses_on_the_same_face() {
	let topology = Solid::cylinder(1.0, DVec3::Z).topology().expect("cylinder topology");
	let seam = topology.edges.iter().find(|edge| edge.incidents.len() == 2 && edge.incidents[0].face == edge.incidents[1].face).expect("cylinder side seam");

	assert!(!seam.is_boundary());
	assert!(!seam.is_non_manifold());
	assert_ne!(seam.incidents[0].edge_use, seam.incidents[1].edge_use);
}

#[test]
fn cut_face_exposes_outer_and_inner_loops_in_order() {
	let block = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let hole = Solid::cylinder(2.0, DVec3::Z * 10.0).translate(DVec3::new(5.0, 5.0, 0.0));
	let cut: Solid = (&block - &hole).build().expect("through hole");
	let topology = cut.topology().expect("cut topology");
	let face = topology.faces.iter().find(|face| face.boundary_loops.len() == 2).expect("planar face with hole");

	assert_eq!(face.boundary_loops.iter().filter(|boundary_loop| boundary_loop.is_outer).count(), 1);
	assert_eq!(face.boundary_loops.iter().filter(|boundary_loop| !boundary_loop.is_outer).count(), 1);
	assert!(face.boundary_loops.iter().all(|boundary_loop| !boundary_loop.edges.is_empty()));
}

#[test]
fn multi_shell_solid_normalizes_cavity_orientation_and_volume() {
	let outer = closed_shell(DVec3::ZERO, DVec3::splat(10.0));
	let cavity = closed_shell(DVec3::splat(2.0), DVec3::splat(8.0));
	let solid = Solid::try_from_shells([&outer, &cavity]).expect("nested multi-shell solid");
	let topology = solid.topology().expect("multi-shell topology");

	assert!(solid.is_valid());
	assert!((solid.volume() - 784.0).abs() < 1.0e-6, "unexpected volume {}", solid.volume());
	assert_eq!(topology.shells.len(), 2);
	assert_eq!(topology.shells[0].role, TopologyShellRole::Outer);
	assert_eq!(topology.shells[1].role, TopologyShellRole::Cavity);
	assert!(topology.shells.iter().all(|shell| shell.is_closed));
	assert_ne!(topology.shells[0].orientation, topology.shells[1].orientation);
	assert!(matches!(topology.shells[0].orientation, TopologyOrientation::Forward | TopologyOrientation::Reversed));
}

#[test]
fn multi_shell_solid_rejects_outside_cavity() {
	let outer = closed_shell(DVec3::ZERO, DVec3::splat(10.0));
	let outside = closed_shell(DVec3::splat(20.0), DVec3::splat(21.0));
	let error = match Solid::try_from_shells([&outer, &outside]) {
		Ok(_) => panic!("outside cavity must fail"),
		Err(error) => error,
	};

	assert!(matches!(error, Error::SolidificationFailed(SolidificationFailure::CavityNotContained { shell_index: 1 })));
}

#[test]
fn multi_shell_solid_rejects_empty_and_open_shell_sets() {
	let empty = solidification_error(Solid::try_from_shells(std::iter::empty::<&Shell>()), "empty shell set must fail");
	assert!(matches!(empty, Error::SolidificationFailed(SolidificationFailure::EmptyShellSet)));

	let open = open_shell(DVec3::ZERO, DVec3::ONE);
	let error = solidification_error(Solid::try_from_shells([&open]), "open outer shell must fail");
	assert!(matches!(error, Error::SolidificationFailed(SolidificationFailure::OpenConstituentShell { shell_index: 0, boundary_edge_count: 4 })));
}

#[test]
fn multi_shell_solid_rejects_touching_outer_and_nested_cavities() {
	let outer = closed_shell(DVec3::ZERO, DVec3::splat(10.0));
	let touching = closed_shell(DVec3::ZERO, DVec3::splat(8.0));
	let error = solidification_error(Solid::try_from_shells([&outer, &touching]), "touching outer and cavity shells must fail");
	assert!(matches!(error, Error::SolidificationFailed(SolidificationFailure::ShellIntersection { first_shell_index: 0, second_shell_index: 1 })));

	let first_cavity = closed_shell(DVec3::splat(2.0), DVec3::splat(8.0));
	let nested_cavity = closed_shell(DVec3::splat(3.0), DVec3::splat(4.0));
	let error = solidification_error(Solid::try_from_shells([&outer, &first_cavity, &nested_cavity]), "cavity volumes may not overlap");
	assert!(matches!(error, Error::SolidificationFailed(SolidificationFailure::ShellIntersection { first_shell_index: 1, second_shell_index: 2 })));
}

#[test]
fn mixed_brep_round_trip_preserves_order_and_classification() {
	let first = BrepBody::Shell(open_shell(DVec3::ZERO, DVec3::ONE));
	let second = BrepBody::Solid(Solid::cube(DVec3::splat(3.0), DVec3::splat(4.0)));
	let third = BrepBody::Shell(closed_shell(DVec3::splat(6.0), DVec3::splat(7.0)));
	let source = [first, second, third];
	let mut payload = Vec::new();

	BrepBody::write_brep(&source, &mut payload).expect("write mixed BRep");
	let decoded = BrepBody::read_brep(&mut payload.as_slice()).expect("read mixed BRep");

	assert_eq!(body_kinds(&decoded), ["shell", "solid", "shell"]);
	let BrepBody::Shell(first) = &decoded[0] else { unreachable!() };
	let BrepBody::Shell(third) = &decoded[2] else { unreachable!() };
	assert!(!first.is_closed());
	assert!(third.is_closed());
}

#[test]
fn mixed_step_round_trip_preserves_order_and_explicit_shells() {
	let first = BrepBody::Shell(open_shell(DVec3::ZERO, DVec3::ONE));
	let second = BrepBody::Solid(Solid::cube(DVec3::splat(3.0), DVec3::splat(4.0)));
	let third = BrepBody::Shell(closed_shell(DVec3::splat(6.0), DVec3::splat(7.0)));
	let source = [first, second, third];
	let mut payload = Vec::new();

	BrepBody::write_step(&source, &mut payload).expect("write mixed STEP");
	let decoded = BrepBody::read_step(&mut payload.as_slice()).expect("read mixed STEP");

	assert_eq!(body_kinds(&decoded), ["shell", "solid", "shell"]);
	let BrepBody::Shell(first) = &decoded[0] else { unreachable!() };
	let BrepBody::Shell(third) = &decoded[2] else { unreachable!() };
	assert!(!first.is_closed());
	assert!(third.is_closed());
}

#[test]
fn mixed_step_order_restoration_keeps_order_within_each_body_kind() {
	let source = [BrepBody::Shell(open_shell(DVec3::ZERO, DVec3::ONE)), BrepBody::Solid(Solid::cube(DVec3::splat(10.0), DVec3::splat(11.0))), BrepBody::Shell(open_shell(DVec3::splat(20.0), DVec3::splat(21.0))), BrepBody::Solid(Solid::cube(DVec3::splat(30.0), DVec3::splat(31.0)))];
	let mut payload = Vec::new();
	BrepBody::write_step(&source, &mut payload).expect("write mixed STEP");
	let decoded = BrepBody::read_step(&mut payload.as_slice()).expect("read mixed STEP");

	assert_eq!(body_kinds(&decoded), ["shell", "solid", "shell", "solid"]);
	for (body, expected) in decoded.iter().zip([0.0, 10.0, 20.0, 30.0]) {
		assert!((body_min_x(body) - expected).abs() < 1.0e-6);
	}
}

#[test]
fn shell_step_api_keeps_closed_shell_as_shell() {
	let source = closed_shell(DVec3::ZERO, DVec3::ONE);
	let mut payload = Vec::new();
	Shell::write_step([&source], &mut payload).expect("write shell STEP");
	let decoded = Shell::read_step(&mut payload.as_slice()).expect("read shell STEP");

	assert_eq!(decoded.len(), 1);
	assert!(decoded[0].is_closed());
}

#[test]
fn multi_shell_solid_round_trips_through_body_brep() {
	let outer = closed_shell(DVec3::ZERO, DVec3::splat(10.0));
	let cavity = closed_shell(DVec3::splat(2.0), DVec3::splat(8.0));
	let source = [BrepBody::Solid(Solid::try_from_shells([&outer, &cavity]).expect("multi-shell solid"))];
	let mut payload = Vec::new();

	BrepBody::write_brep(&source, &mut payload).expect("write body BRep");
	let decoded = BrepBody::read_brep(&mut payload.as_slice()).expect("read body BRep");
	let [BrepBody::Solid(solid)] = decoded.as_slice() else {
		panic!("multi-shell solid classification changed");
	};

	assert_eq!(solid.topology().expect("round-trip topology").shells.len(), 2);
	assert!((solid.volume() - 784.0).abs() < 1.0e-6);
}

#[test]
fn multi_shell_solid_round_trips_through_body_step() {
	let outer = closed_shell(DVec3::ZERO, DVec3::splat(10.0));
	let cavity = closed_shell(DVec3::splat(2.0), DVec3::splat(8.0));
	let source = [BrepBody::Solid(Solid::try_from_shells([&outer, &cavity]).expect("multi-shell solid"))];
	let mut payload = Vec::new();

	BrepBody::write_step(&source, &mut payload).expect("write body STEP");
	let decoded = BrepBody::read_step(&mut payload.as_slice()).expect("read body STEP");
	let [BrepBody::Solid(solid)] = decoded.as_slice() else {
		panic!("multi-shell solid classification changed");
	};

	assert_eq!(solid.topology().expect("round-trip topology").shells.len(), 2);
	assert!((solid.volume() - 784.0).abs() < 1.0e-6);
}

#[cfg(feature = "color")]
#[test]
fn mixed_body_writers_keep_solid_color() {
	use cadrum::Color;

	let red = Color::from_str("#ff0000").expect("red");
	let make_source = || [BrepBody::Shell(open_shell(DVec3::ZERO, DVec3::ONE)), BrepBody::Solid(Solid::cube(DVec3::splat(3.0), DVec3::splat(4.0)).color(red))];

	let source = make_source();
	let mut brep = Vec::new();
	BrepBody::write_brep(&source, &mut brep).expect("write colored mixed BRep");
	let decoded = BrepBody::read_brep(&mut brep.as_slice()).expect("read colored mixed BRep");
	let BrepBody::Solid(solid) = &decoded[1] else { panic!("solid order changed") };
	assert_eq!(solid.colormap().get(&solid.id()), Some(&red));

	let source = make_source();
	let mut step = Vec::new();
	BrepBody::write_step(&source, &mut step).expect("write colored mixed STEP");
	let decoded = BrepBody::read_step(&mut step.as_slice()).expect("read colored mixed STEP");
	let BrepBody::Solid(solid) = &decoded[1] else { panic!("solid order changed") };
	assert_eq!(solid.colormap().get(&solid.id()), Some(&red));
}
