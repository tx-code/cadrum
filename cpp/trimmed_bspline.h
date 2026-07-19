#pragma once

#include <cstdint>
#include <memory>

class TopoDS_Edge;
class TopoDS_Face;

namespace cadrum {

using TopoDS_Edge = ::TopoDS_Edge;
using TopoDS_Face = ::TopoDS_Face;

struct BSplineCurveData;
struct TrimmedFaceData;

std::unique_ptr<TopoDS_Edge> make_exact_bspline_edge(
    const BSplineCurveData& data);
BSplineCurveData edge_bspline_curve(const TopoDS_Edge& edge);

std::unique_ptr<TopoDS_Face> make_trimmed_bspline_face(
    const TrimmedFaceData& data,
    uint32_t& out_status);
TrimmedFaceData face_trimmed_bspline_data(const TopoDS_Face& face);

} // namespace cadrum
