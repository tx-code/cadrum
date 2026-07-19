#include "cadrum/src/occt/ffi.rs.h"
#include "bspline_internal.h"

#include <Standard_Failure.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColStd_Array2OfReal.hxx>

namespace cadrum {

Handle(Geom_BSplineSurface) make_bspline_surface(
    const BSplineSurfaceData& data)
{
    try {
        const auto& control_points = data.control_points;
        const auto& weights = data.weights;
        const auto& u_knots = data.u_knots;
        const auto& u_multiplicities = data.u_multiplicities;
        const auto& v_knots = data.v_knots;
        const auto& v_multiplicities = data.v_multiplicities;
        const uint32_t u_count = data.u_count;
        const uint32_t v_count = data.v_count;
        const size_t pole_count = static_cast<size_t>(u_count) * v_count;
        if (u_count < 2 || v_count < 2 || control_points.size() != pole_count * 3) return {};
        if (!weights.empty() && weights.size() != pole_count) return {};
        if (u_knots.empty() || v_knots.empty()) return {};
        if (u_knots.size() != u_multiplicities.size() ||
            v_knots.size() != v_multiplicities.size()) return {};

        TColgp_Array2OfPnt poles(1, static_cast<int>(u_count), 1, static_cast<int>(v_count));
        for (size_t v = 0; v < v_count; ++v) {
            for (size_t u = 0; u < u_count; ++u) {
                const size_t index = (v * u_count + u) * 3;
                poles.SetValue(
                    static_cast<int>(u + 1),
                    static_cast<int>(v + 1),
                    gp_Pnt(control_points[index], control_points[index + 1], control_points[index + 2]));
            }
        }

        TColStd_Array1OfReal u_knot_array(1, static_cast<int>(u_knots.size()));
        TColStd_Array1OfReal v_knot_array(1, static_cast<int>(v_knots.size()));
        TColStd_Array1OfInteger u_mult_array(1, static_cast<int>(u_multiplicities.size()));
        TColStd_Array1OfInteger v_mult_array(1, static_cast<int>(v_multiplicities.size()));
        for (size_t i = 0; i < u_knots.size(); ++i) {
            u_knot_array.SetValue(static_cast<int>(i + 1), u_knots[i]);
            u_mult_array.SetValue(static_cast<int>(i + 1), static_cast<int>(u_multiplicities[i]));
        }
        for (size_t i = 0; i < v_knots.size(); ++i) {
            v_knot_array.SetValue(static_cast<int>(i + 1), v_knots[i]);
            v_mult_array.SetValue(static_cast<int>(i + 1), static_cast<int>(v_multiplicities[i]));
        }

        if (weights.empty()) {
            return new Geom_BSplineSurface(
                poles, u_knot_array, v_knot_array, u_mult_array, v_mult_array,
                static_cast<int>(data.u_degree), static_cast<int>(data.v_degree),
                data.u_periodic, data.v_periodic);
        }

        TColStd_Array2OfReal weight_array(1, static_cast<int>(u_count), 1, static_cast<int>(v_count));
        for (size_t v = 0; v < v_count; ++v) {
            for (size_t u = 0; u < u_count; ++u) {
                weight_array.SetValue(static_cast<int>(u + 1), static_cast<int>(v + 1), weights[v * u_count + u]);
            }
        }
        return new Geom_BSplineSurface(
            poles, weight_array, u_knot_array, v_knot_array, u_mult_array, v_mult_array,
            static_cast<int>(data.u_degree), static_cast<int>(data.v_degree),
            data.u_periodic, data.v_periodic);
    } catch (const Standard_Failure&) {
        return {};
    }
}

} // namespace cadrum
