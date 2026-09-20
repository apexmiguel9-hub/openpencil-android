//! Planning/live screen-route context shared by fan-out prompts and wiring.

use super::*;

/// Name/route context for loop continuation prompts. Authored top-level
/// screens are runtime truth regardless of width; unmarked roots still use
/// navigation's conservative screen-shape bands.
pub(crate) fn screen_route_inventory(state: &EditorState) -> Vec<(String, String)> {
    let screens = collect_prompt_live_candidates(state);
    if screens.len() < 2 {
        return Vec::new();
    }
    route_inventory_for_candidates(&screens)
}

/// Classic fan-out uses normalized planning groups, merged with existing
/// screens. Synthetic loop plans have no distinct screen roots and fall back
/// to the live inventory above.
pub(crate) fn prompt_screen_route_inventory(
    plan: &crate::plan::OrchestratorPlan,
    state: &EditorState,
) -> Vec<(String, String)> {
    plan_route_candidates(plan, state)
        .map(|candidates| route_inventory_for_candidates(&candidates))
        .unwrap_or_else(|| screen_route_inventory(state))
}

/// Persist the same merged assignments that fan-out prompts see. Called once
/// after per-screen scaffold roots exist and before their workers run.
pub(crate) fn ensure_planned_screen_routes(
    sink: &mut dyn DocSink,
    plan: &crate::plan::OrchestratorPlan,
) {
    let Some(candidates) = plan_route_candidates(plan, sink.state()) else {
        return;
    };
    for (node_id, path) in assign_screen_paths(&candidates) {
        sink.apply(EditorCommand::PatchNodeData {
            node_id: NodeId::new(node_id),
            patch_json: format!(r#"{{"screen":"{path}"}}"#),
            page_id: None,
        });
    }
}

fn route_inventory_for_candidates(screens: &[ScreenCandidate]) -> Vec<(String, String)> {
    let assignments = assign_screen_paths(screens);
    let assigned: HashMap<&str, &str> = assignments
        .iter()
        .map(|(id, path)| (id.as_str(), path.as_str()))
        .collect();
    screens
        .iter()
        .filter_map(|screen| {
            screen
                .existing_path
                .clone()
                .or_else(|| {
                    assigned
                        .get(screen.id.as_str())
                        .map(|path| path.to_string())
                })
                .map(|path| (screen.name.clone(), path))
        })
        .collect()
}

fn plan_route_candidates(
    plan: &crate::plan::OrchestratorPlan,
    state: &EditorState,
) -> Option<Vec<ScreenCandidate>> {
    let groups = crate::screen_groups::group_subtasks_by_screen(&plan.subtasks);
    if groups.len() < 2 {
        return None;
    }
    let planned = groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let root_id = group
                .indices
                .iter()
                .filter_map(|subtask_index| plan.subtasks.get(*subtask_index))
                .find_map(|subtask| subtask.parent_frame_id.clone())
                .unwrap_or_else(|| format!("planned-screen-{index}"));
            (root_id, group.screen.clone())
        })
        .collect::<Vec<_>>();
    let distinct_ids = planned
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<BTreeSet<_>>();
    if distinct_ids.len() < 2 || distinct_ids.len() != planned.len() {
        return None;
    }

    let planned_names = planned.iter().cloned().collect::<HashMap<String, String>>();
    let shaped = collect_screen_candidates(state)
        .into_iter()
        .map(|candidate| (candidate.id.clone(), candidate))
        .collect::<HashMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for node in state.active_children() {
        let PenNode::Frame(frame) = node else {
            continue;
        };
        let id = frame.base.id.clone();
        if let Some(name) = planned_names.get(&id) {
            seen.insert(id.clone());
            candidates.push(ScreenCandidate {
                id,
                name: name.clone(),
                existing_path: frame.screen.clone(),
            });
        } else if let Some(candidate) = shaped.get(&id) {
            seen.insert(id);
            candidates.push(ScreenCandidate {
                id: candidate.id.clone(),
                name: candidate.name.clone(),
                existing_path: candidate.existing_path.clone(),
            });
        } else if frame.screen.is_some() {
            seen.insert(id.clone());
            candidates.push(ScreenCandidate {
                id,
                name: frame
                    .base
                    .name
                    .clone()
                    .unwrap_or_else(|| frame.base.id.clone()),
                existing_path: frame.screen.clone(),
            });
        }
    }
    for (id, name) in planned {
        if seen.insert(id.clone()) {
            candidates.push(ScreenCandidate {
                id,
                name,
                existing_path: None,
            });
        }
    }
    Some(candidates)
}

pub(super) fn collect_prompt_live_candidates(state: &EditorState) -> Vec<ScreenCandidate> {
    let shaped = collect_screen_candidates(state)
        .into_iter()
        .map(|candidate| (candidate.id.clone(), candidate))
        .collect::<HashMap<_, _>>();
    state
        .active_children()
        .iter()
        .filter_map(|node| {
            let PenNode::Frame(frame) = node else {
                return None;
            };
            shaped.get(&frame.base.id).map_or_else(
                || {
                    frame.screen.as_ref().map(|path| ScreenCandidate {
                        id: frame.base.id.clone(),
                        name: frame
                            .base
                            .name
                            .clone()
                            .unwrap_or_else(|| frame.base.id.clone()),
                        existing_path: Some(path.clone()),
                    })
                },
                |candidate| {
                    Some(ScreenCandidate {
                        id: candidate.id.clone(),
                        name: candidate.name.clone(),
                        existing_path: candidate.existing_path.clone(),
                    })
                },
            )
        })
        .collect()
}
