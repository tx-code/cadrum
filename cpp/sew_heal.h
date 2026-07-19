#pragma once

#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>

#include <memory>
#include <vector>

namespace cadrum {

using TopoDS_Shape = ::TopoDS_Shape;
using TopoDS_Face = ::TopoDS_Face;
using TopoDS_Edge = ::TopoDS_Edge;

struct ShapeRepairData;

std::unique_ptr<TopoDS_Shape> sew_faces_with_report(
    const std::vector<TopoDS_Face>& faces,
    double tolerance,
    double maximum_tolerance,
    ShapeRepairData& report);

std::unique_ptr<TopoDS_Shape> heal_shell_with_report(
    const TopoDS_Shape& shape,
    double tolerance,
    double maximum_tolerance,
    ShapeRepairData& report);

} // namespace cadrum
