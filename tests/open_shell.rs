use cadrum::{DVec3, Shell, Solid};

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
fn disconnected_faces_do_not_fabricate_one_shell() {
	let left = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let right = Solid::cube(DVec3::splat(3.0), DVec3::splat(4.0));
	let faces = [left.iter_face().next().expect("left face"), right.iter_face().next().expect("right face")];

	assert!(Shell::sew(faces, 1.0e-7).is_err());
}
