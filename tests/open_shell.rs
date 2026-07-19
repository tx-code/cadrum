use cadrum::{BrepBody, DVec3, Error, Face, Shell, Solid, SolidificationFailure};

#[test]
fn sewing_five_cube_faces_builds_a_valid_open_shell() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
	let faces: Vec<_> = cube.iter_face().take(5).collect();
	let shell = Shell::sew(faces, 1.0e-7).expect("open shell sewing");

	assert!(shell.is_valid());
	assert!(!shell.is_closed());
	assert_eq!(shell.iter_face().count(), 5);
	assert_eq!(shell.boundary_edge_count(), 4);
}

#[test]
fn sewing_all_cube_faces_builds_a_closed_shell() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let shell = Shell::sew(cube.iter_face(), 1.0e-7).expect("closed shell sewing");

	assert!(shell.is_valid());
	assert!(shell.is_closed());
	assert_eq!(shell.boundary_edge_count(), 0);
	assert_eq!(shell.iter_face().count(), 6);
}

#[test]
fn closed_shell_promotes_to_one_valid_positive_volume_solid() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
	let shell = Shell::sew(cube.iter_face(), 1.0e-7).expect("closed shell sewing");
	let solid = shell.try_to_solid().expect("closed shell solidification");

	assert!(solid.is_valid());
	assert!((solid.volume() - 24.0).abs() < 1.0e-9);
	let mut brep = Vec::new();
	Solid::write_brep([&solid], &mut brep).expect("solid BRep write");
	let bodies = BrepBody::read_brep(&mut std::io::Cursor::new(brep)).expect("solid BRep read");
	assert!(matches!(bodies.as_slice(), [BrepBody::Solid(_)]));
}

#[test]
fn closed_periodic_face_shell_promotes_without_false_non_manifold_rejection() {
	let sphere = Solid::sphere(2.0);
	let mut brep = Vec::new();
	Solid::write_brep([&sphere], &mut brep).expect("sphere BRep write");
	let shells = Shell::read_brep(&mut std::io::Cursor::new(brep)).expect("periodic shell read");
	assert_eq!(shells.len(), 1);
	assert!(shells[0].is_closed());
	assert_eq!(shells[0].boundary_edge_count(), 0);
	let solid = shells[0].try_to_solid().expect("periodic shell solidification");

	assert!(solid.is_valid());
	assert!((solid.volume() - 4.0 / 3.0 * std::f64::consts::PI * 8.0).abs() < 1.0e-9);
}

#[test]
fn open_shell_reports_its_boundary_instead_of_becoming_a_solid() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let shell = Shell::sew(cube.iter_face().take(5), 1.0e-7).expect("open shell sewing");

	let error = match shell.try_to_solid() {
		Ok(_) => panic!("open shell became a solid"),
		Err(error) => error,
	};
	assert!(matches!(error, Error::SolidificationFailed(SolidificationFailure::OpenShell { boundary_edge_count: 4 })));
}

#[test]
fn step_open_face_set_is_not_silently_promoted() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let mut step = Vec::new();
	Face::write_step(cube.iter_face().take(5), &mut step).expect("open STEP face write");

	let solids = Solid::read_step(&mut std::io::Cursor::new(&step)).expect("STEP solid read");
	assert!(solids.is_empty());
	let faces = Face::read_step(&mut std::io::Cursor::new(step)).expect("STEP face read");
	assert_eq!(faces.len(), 5);
}

#[test]
fn step_closed_face_set_still_recovers_one_valid_solid() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
	let mut step = Vec::new();
	Face::write_step(cube.iter_face(), &mut step).expect("closed STEP face write");

	let solids = Solid::read_step(&mut std::io::Cursor::new(step)).expect("STEP solid read");
	assert_eq!(solids.len(), 1);
	assert!(solids[0].is_valid());
	assert!((solids[0].volume() - 24.0).abs() < 1.0e-9);
}

#[test]
fn shell_brep_roundtrip_preserves_open_topology() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let shell = Shell::sew(cube.iter_face().take(5), 1.0e-7).expect("open shell sewing");
	let mut brep = Vec::new();
	Shell::write_brep([&shell], &mut brep).expect("shell BRep write");
	let shells = Shell::read_brep(&mut std::io::Cursor::new(brep)).expect("shell BRep read");

	assert_eq!(shells.len(), 1);
	assert!(shells[0].is_valid());
	assert!(!shells[0].is_closed());
	assert_eq!(shells[0].boundary_edge_count(), 4);
	assert_eq!(shells[0].iter_face().count(), 5);
}

#[test]
fn brep_body_reader_preserves_open_shell_classification() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let shell = Shell::sew(cube.iter_face().take(5), 1.0e-7).expect("open shell sewing");
	let mut brep = Vec::new();
	Shell::write_brep([&shell], &mut brep).expect("shell BRep write");
	let bodies = BrepBody::read_brep(&mut std::io::Cursor::new(brep)).expect("BRep body read");

	assert_eq!(bodies.len(), 1);
	let BrepBody::Shell(shell) = &bodies[0] else {
		panic!("open shell was promoted to a solid");
	};
	assert!(shell.is_valid());
	assert!(!shell.is_closed());
}

#[test]
fn brep_body_reader_preserves_solid_classification() {
	let solid = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let mut brep = Vec::new();
	Solid::write_brep([&solid], &mut brep).expect("solid BRep write");
	let bodies = BrepBody::read_brep(&mut std::io::Cursor::new(brep)).expect("BRep body read");

	assert_eq!(bodies.len(), 1);
	assert!(matches!(bodies[0], BrepBody::Solid(_)));
}

#[test]
fn open_shell_mesh_preserves_face_mapping_and_boundary_wire() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let shell = Shell::sew(cube.iter_face().take(5), 1.0e-7).expect("open shell sewing");
	let mesh = Shell::mesh([&shell], Default::default()).expect("open shell mesh");

	assert!(!mesh.vertices.is_empty());
	assert!(!mesh.indices.is_empty());
	assert_eq!(mesh.face_indices.len(), mesh.indices.len() / 3);
	assert!(mesh.face_indices.iter().all(|&face| face < 5));
	assert_eq!(mesh.face_indices.iter().copied().max(), Some(4));
	assert!(!mesh.edges.is_empty());
}

#[test]
fn disconnected_faces_do_not_fabricate_one_shell() {
	let left = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let right = Solid::cube(DVec3::splat(3.0), DVec3::splat(4.0));
	let faces = [left.iter_face().next().expect("left face"), right.iter_face().next().expect("right face")];

	assert!(Shell::sew(faces, 1.0e-7).is_err());
}
