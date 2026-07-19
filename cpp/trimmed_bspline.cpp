#include "cadrum/src/occt/ffi.rs.h"
#include "bspline_internal.h"

#include <TopoDS.hxx>
#include <TopoDS_Wire.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <NCollection_IndexedMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>

#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomConvert.hxx>
#include <Geom2dConvert.hxx>

#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepLib_CheckCurveOnSurface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>

#include <NCollection_Array1.hxx>
#include <Precision.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>

#include <algorithm>
#include <array>
#include <cmath>
#include <memory>
#include <utility>
#include <vector>

namespace cadrum {

static Handle(Geom_BSplineCurve) make_bspline_curve3(
    const BSplineCurveData& data)
{
    if (!data.success || data.dimension != 3 ||
        data.control_points.size() < 6 || data.control_points.size() % 3 != 0 ||
        data.knots.size() < 2 || data.knots.size() != data.multiplicities.size()) return {};
    const int count = static_cast<int>(data.control_points.size() / 3);
    if (data.degree == 0 || data.degree >= static_cast<uint32_t>(count) ||
        (!data.weights.empty() && data.weights.size() != static_cast<size_t>(count))) return {};

    NCollection_Array1<gp_Pnt> poles(1, count);
    for (int i = 0; i < count; ++i) {
        poles.SetValue(i + 1, gp_Pnt(
            data.control_points[static_cast<size_t>(i) * 3],
            data.control_points[static_cast<size_t>(i) * 3 + 1],
            data.control_points[static_cast<size_t>(i) * 3 + 2]));
    }
    TColStd_Array1OfReal knots(1, static_cast<int>(data.knots.size()));
    TColStd_Array1OfInteger multiplicities(1, static_cast<int>(data.multiplicities.size()));
    for (size_t i = 0; i < data.knots.size(); ++i) {
        knots.SetValue(static_cast<int>(i + 1), data.knots[i]);
        multiplicities.SetValue(static_cast<int>(i + 1), static_cast<int>(data.multiplicities[i]));
    }
    if (data.weights.empty()) {
        return new Geom_BSplineCurve(
            poles, knots, multiplicities, static_cast<int>(data.degree), data.periodic);
    }
    TColStd_Array1OfReal weights(1, count);
    for (int i = 0; i < count; ++i) {
        weights.SetValue(i + 1, data.weights[static_cast<size_t>(i)]);
    }
    return new Geom_BSplineCurve(
        poles, weights, knots, multiplicities,
        static_cast<int>(data.degree), data.periodic);
}

static Handle(Geom2d_BSplineCurve) make_bspline_curve2(
    const BSplineCurveData& data)
{
    if (!data.success || data.dimension != 2 ||
        data.control_points.size() < 4 || data.control_points.size() % 2 != 0 ||
        data.knots.size() < 2 || data.knots.size() != data.multiplicities.size()) return {};
    const int count = static_cast<int>(data.control_points.size() / 2);
    if (data.degree == 0 || data.degree >= static_cast<uint32_t>(count) ||
        (!data.weights.empty() && data.weights.size() != static_cast<size_t>(count))) return {};

    NCollection_Array1<gp_Pnt2d> poles(1, count);
    for (int i = 0; i < count; ++i) {
        poles.SetValue(i + 1, gp_Pnt2d(
            data.control_points[static_cast<size_t>(i) * 2],
            data.control_points[static_cast<size_t>(i) * 2 + 1]));
    }
    TColStd_Array1OfReal knots(1, static_cast<int>(data.knots.size()));
    TColStd_Array1OfInteger multiplicities(1, static_cast<int>(data.multiplicities.size()));
    for (size_t i = 0; i < data.knots.size(); ++i) {
        knots.SetValue(static_cast<int>(i + 1), data.knots[i]);
        multiplicities.SetValue(static_cast<int>(i + 1), static_cast<int>(data.multiplicities[i]));
    }
    if (data.weights.empty()) {
        return new Geom2d_BSplineCurve(
            poles, knots, multiplicities, static_cast<int>(data.degree), data.periodic);
    }
    TColStd_Array1OfReal weights(1, count);
    for (int i = 0; i < count; ++i) {
        weights.SetValue(i + 1, data.weights[static_cast<size_t>(i)]);
    }
    return new Geom2d_BSplineCurve(
        poles, weights, knots, multiplicities,
        static_cast<int>(data.degree), data.periodic);
}

static BSplineCurveData bspline_curve3_data(
    const Handle(Geom_Curve)& source, double first, double last)
{
    BSplineCurveData data{};
    data.dimension = 3;
    data.first = first;
    data.last = last;
    if (source.IsNull()) return data;
    try {
        Handle(Geom_BSplineCurve) curve = Handle(Geom_BSplineCurve)::DownCast(source);
        if (curve.IsNull()) {
            Handle(Geom_TrimmedCurve) trimmed = new Geom_TrimmedCurve(source, first, last);
            curve = GeomConvert::CurveToBSplineCurve(trimmed);
        }
        if (curve.IsNull()) return data;
        data.degree = static_cast<uint32_t>(curve->Degree());
        data.periodic = curve->IsPeriodic();
        const bool rational = curve->IsRational();
        for (int i = 1; i <= curve->NbPoles(); ++i) {
            const gp_Pnt point = curve->Pole(i);
            data.control_points.push_back(point.X());
            data.control_points.push_back(point.Y());
            data.control_points.push_back(point.Z());
            if (rational) data.weights.push_back(curve->Weight(i));
        }
        for (int i = 1; i <= curve->NbKnots(); ++i) {
            data.knots.push_back(curve->Knot(i));
            data.multiplicities.push_back(static_cast<uint32_t>(curve->Multiplicity(i)));
        }
        data.success = true;
    } catch (const Standard_Failure&) {
        data.success = false;
    }
    return data;
}

static BSplineCurveData bspline_curve2_data(
    const Handle(Geom2d_Curve)& source, double first, double last)
{
    BSplineCurveData data{};
    data.dimension = 2;
    data.first = first;
    data.last = last;
    if (source.IsNull()) return data;
    try {
        Handle(Geom2d_BSplineCurve) curve = Handle(Geom2d_BSplineCurve)::DownCast(source);
        if (curve.IsNull()) {
            Handle(Geom2d_TrimmedCurve) trimmed = new Geom2d_TrimmedCurve(source, first, last);
            curve = Geom2dConvert::CurveToBSplineCurve(trimmed);
        }
        if (curve.IsNull()) return data;
        data.degree = static_cast<uint32_t>(curve->Degree());
        data.periodic = curve->IsPeriodic();
        const bool rational = curve->IsRational();
        for (int i = 1; i <= curve->NbPoles(); ++i) {
            const gp_Pnt2d point = curve->Pole(i);
            data.control_points.push_back(point.X());
            data.control_points.push_back(point.Y());
            if (rational) data.weights.push_back(curve->Weight(i));
        }
        for (int i = 1; i <= curve->NbKnots(); ++i) {
            data.knots.push_back(curve->Knot(i));
            data.multiplicities.push_back(static_cast<uint32_t>(curve->Multiplicity(i)));
        }
        data.success = true;
    } catch (const Standard_Failure&) {
        data.success = false;
    }
    return data;
}

std::unique_ptr<TopoDS_Edge> make_exact_bspline_edge(
    const BSplineCurveData& data)
{
    try {
        Handle(Geom_BSplineCurve) curve = make_bspline_curve3(data);
        if (curve.IsNull() || !std::isfinite(data.first) ||
            !std::isfinite(data.last) || data.first >= data.last) return nullptr;
        BRepBuilderAPI_MakeEdge maker(curve, data.first, data.last);
        if (!maker.IsDone()) return nullptr;
        return std::make_unique<TopoDS_Edge>(maker.Edge());
    } catch (const Standard_Failure&) {
        return nullptr;
    }
}

BSplineCurveData edge_bspline_curve(const TopoDS_Edge& edge) {
    try {
        double first = 0.0;
        double last = 0.0;
        Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
        return bspline_curve3_data(curve, first, last);
    } catch (const Standard_Failure&) {
        return {};
    }
}

static bool same_range(double first1, double last1, double first2, double last2) {
    const auto close = [](double left, double right) {
        const double scale = std::max({1.0, std::abs(left), std::abs(right)});
        return std::isfinite(left) && std::isfinite(right)
            && std::abs(left - right) <= 1.0e-12 * scale;
    };
    return close(first1, first2) && close(last1, last2);
}

std::unique_ptr<TopoDS_Face> make_trimmed_bspline_face(
    const TrimmedFaceData& data,
    uint32_t& out_status)
{
    /* 1=input, 2=surface, 3=edge, 4=p-curve, 5=seam, 6=wire, 7=consistency, 8=face, 9=kernel */
    out_status = 0;
    const auto fail = [&](uint32_t status) {
        out_status = status;
        return std::unique_ptr<TopoDS_Face>{};
    };
    if (!data.success || !std::isfinite(data.tolerance) || data.tolerance <= 0.0 ||
        data.edges.empty() || data.loops.empty()) return fail(1);
    try {
        Handle(Geom_BSplineSurface) surface = make_bspline_surface(data.surface);
        if (surface.IsNull()) return fail(2);

        std::vector<TopoDS_Edge> edges;
        std::vector<Handle(Geom_BSplineCurve)> curves;
        std::vector<std::array<double, 2>> ranges;
        edges.reserve(data.edges.size());
        curves.reserve(data.edges.size());
        ranges.reserve(data.edges.size());
        for (const auto& edge_data : data.edges) {
            Handle(Geom_BSplineCurve) curve = make_bspline_curve3(edge_data);
            if (curve.IsNull() || !std::isfinite(edge_data.first) ||
                !std::isfinite(edge_data.last) || edge_data.first >= edge_data.last) return fail(3);
            BRepBuilderAPI_MakeEdge maker(curve, edge_data.first, edge_data.last);
            if (!maker.IsDone()) return fail(3);
            edges.push_back(maker.Edge());
            curves.push_back(curve);
            ranges.push_back({edge_data.first, edge_data.last});
        }

        std::vector<std::vector<Handle(Geom2d_BSplineCurve)>> pcurves(edges.size());
        std::vector<std::vector<bool>> orientations(edges.size());
        for (const auto& loop : data.loops) {
            if (loop.edges.empty()) return fail(1);
            for (const auto& edge_use : loop.edges) {
                if (edge_use.edge >= edges.size()) return fail(1);
                Handle(Geom2d_BSplineCurve) pcurve = make_bspline_curve2(edge_use.pcurve);
                if (pcurve.IsNull() ||
                    !same_range(ranges[edge_use.edge][0], ranges[edge_use.edge][1],
                                edge_use.pcurve.first, edge_use.pcurve.last)) return fail(4);
                pcurves[edge_use.edge].push_back(pcurve);
                orientations[edge_use.edge].push_back(edge_use.reversed);
                if (pcurves[edge_use.edge].size() > 2) return fail(5);
            }
        }

        BRep_Builder builder;
        const TopLoc_Location location;
        for (size_t i = 0; i < edges.size(); ++i) {
            if (pcurves[i].empty()) return fail(1);
            if (pcurves[i].size() == 1) {
                builder.UpdateEdge(edges[i], pcurves[i][0], surface, location, data.tolerance);
            } else {
                if (orientations[i][0] == orientations[i][1]) return fail(5);
                const size_t forward = orientations[i][0] ? 1 : 0;
                const size_t reversed = 1 - forward;
                builder.UpdateEdge(edges[i], pcurves[i][forward], pcurves[i][reversed], surface, location, data.tolerance);
            }
            builder.Range(edges[i], surface, location, ranges[i][0], ranges[i][1]);
            builder.SameRange(edges[i], true);
            builder.SameParameter(edges[i], true);
        }

        TopoDS_Face face;
        builder.MakeFace(face, surface, data.tolerance);
        for (const auto& loop : data.loops) {
            BRepBuilderAPI_MakeWire wire_maker;
            gp_Pnt loop_start;
            gp_Pnt previous_end;
            bool first_use = true;
            for (const auto& edge_use : loop.edges) {
                const auto& range = ranges[edge_use.edge];
                gp_Pnt start = curves[edge_use.edge]->Value(edge_use.reversed ? range[1] : range[0]);
                gp_Pnt end = curves[edge_use.edge]->Value(edge_use.reversed ? range[0] : range[1]);
                if (first_use) {
                    loop_start = start;
                    first_use = false;
                } else if (previous_end.Distance(start) > data.tolerance) {
                    return fail(6);
                }
                previous_end = end;
                TopoDS_Edge edge = edges[edge_use.edge];
                edge.Orientation(edge_use.reversed ? TopAbs_REVERSED : TopAbs_FORWARD);
                wire_maker.Add(edge);
            }
            if (previous_end.Distance(loop_start) > data.tolerance) return fail(6);
            if (!wire_maker.IsDone()) return fail(6);
            builder.Add(face, wire_maker.Wire());
        }

        for (const auto& edge : edges) {
            BRepLib_CheckCurveOnSurface check(edge, face);
            check.Perform();
            if (!check.IsDone() || !std::isfinite(check.MaxDistance()) ||
                check.MaxDistance() > data.tolerance) return fail(7);
        }
        if (!BRepCheck_Analyzer(face).IsValid() || BRepTools::OuterWire(face).IsNull()) return fail(8);
        return std::make_unique<TopoDS_Face>(face);
    } catch (const Standard_Failure&) {
        return fail(9);
    }
}

TrimmedFaceData face_trimmed_bspline_data(const TopoDS_Face& face) {
    TrimmedFaceData data{};
    try {
        data.surface = face_bspline_surface(face);
        if (!data.surface.success) return data;
        data.tolerance = std::max(BRep_Tool::Tolerance(face), Precision::Confusion());

        NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
        TopExp::MapShapes(face, TopAbs_EDGE, edges);
        if (edges.IsEmpty()) return data;
        for (int i = 1; i <= edges.Extent(); ++i) {
            const TopoDS_Edge edge = TopoDS::Edge(edges(i));
            double first = 0.0;
            double last = 0.0;
            Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
            BSplineCurveData curve_data = bspline_curve3_data(curve, first, last);
            if (!curve_data.success) return {};
            data.edges.push_back(std::move(curve_data));
        }

        const auto append_loop = [&](const TopoDS_Wire& wire) -> bool {
            TrimLoopData loop{};
            for (BRepTools_WireExplorer explorer(wire, face); explorer.More(); explorer.Next()) {
                const TopoDS_Edge edge = explorer.Current();
                const int edge_index = edges.FindIndex(edge);
                if (edge_index < 1) return false;
                double first = 0.0;
                double last = 0.0;
                Handle(Geom2d_Curve) pcurve = BRep_Tool::CurveOnSurface(edge, face, first, last);
                BSplineCurveData pcurve_data = bspline_curve2_data(pcurve, first, last);
                if (!pcurve_data.success) return false;
                TrimEdgeUseData edge_use{};
                edge_use.edge = static_cast<uint32_t>(edge_index - 1);
                edge_use.reversed = edge.Orientation() == TopAbs_REVERSED;
                edge_use.pcurve = std::move(pcurve_data);
                loop.edges.push_back(std::move(edge_use));
            }
            if (loop.edges.empty()) return false;
            data.loops.push_back(std::move(loop));
            return true;
        };

        const TopoDS_Wire outer = BRepTools::OuterWire(face);
        if (outer.IsNull() || !append_loop(outer)) return {};
        for (TopExp_Explorer explorer(face, TopAbs_WIRE); explorer.More(); explorer.Next()) {
            const TopoDS_Wire wire = TopoDS::Wire(explorer.Current());
            if (!wire.IsSame(outer) && !append_loop(wire)) return {};
        }
        data.success = true;
    } catch (const Standard_Failure&) {
        data.success = false;
    }
    return data;
}

} // namespace cadrum
