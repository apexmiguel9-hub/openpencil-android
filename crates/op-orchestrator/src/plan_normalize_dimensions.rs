//! Request-derived root dimensions applied before the remaining plan rules.

use crate::plan::OrchestratorPlan;
use crate::request_dimensions::requested_root_dimensions;
use crate::types::DesignRequest;

/// Apply explicit root dimensions and report whether height was also fixed.
pub(super) fn apply_requested_root_dimensions(
    plan: &mut OrchestratorPlan,
    req: &DesignRequest,
) -> bool {
    let Some(dimensions) = requested_root_dimensions(&req.prompt) else {
        return false;
    };
    let planned_width = plan.root_frame.width;
    plan.root_frame.width = dimensions.width;
    if let Some(height) = dimensions.height {
        plan.root_frame.height = height;
    }
    if (planned_width - dimensions.width).abs() > f64::EPSILON {
        for subtask in &mut plan.subtasks {
            if (subtask.region.width - planned_width).abs() <= 1.0 {
                subtask.region.width = dimensions.width;
            }
        }
    }
    dimensions.height.is_some()
}
