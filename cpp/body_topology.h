#pragma once

#include <TopoDS_Shape.hxx>

#include <memory>
#include <vector>

namespace cadrum {

using TopoDS_Shape = ::TopoDS_Shape;

struct ShapeTopologyData;

/// Flatten only Compound / CompSolid containers, preserving body occurrence
/// order and keeping shells nested in a Solid owned by that Solid.
std::unique_ptr<std::vector<TopoDS_Shape>> decompose_into_brep_bodies(
    const TopoDS_Shape& shape);

/// True when every leaf in a STEP-read shape is already an explicit Solid or
/// Shell. In that case body-mode import must not sew or promote those shells.
bool step_body_types_are_complete(const TopoDS_Shape& shape);

/// Build the ordered, index-addressed topology exchange snapshot for a Solid
/// or Shell without exposing mutable OCCT handles.
ShapeTopologyData shape_topology(const TopoDS_Shape& shape);

} // namespace cadrum
