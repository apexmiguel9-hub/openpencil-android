//! Pure helpers behind the Git host: merge-resolution state assembly,
//! single-hunk patch construction, and conflict-dialog copy. Carved out
//! of the `git_host.rs` spine to keep it under the 800-line cap; pure
//! code motion.

/// Build the interactive [`MergeResolveState`] from a conflict bag
/// — `None` when any conflicted file is not a structured `.op`
/// document (those cannot be resolved per node, so the caller falls
/// back to the conflict dialog).
pub(super) fn build_merge_resolve(
    branch: &str,
    conflicts: &op_git::ConflictBag,
    locale: op_editor_core::Locale,
) -> Option<op_editor_core::MergeResolveState> {
    use op_editor_core::{MergeConflictRow, MergeResolveFile, MergeResolveState};
    let mut files = Vec::new();
    for file in &conflicts.files {
        // A non-`.op` file carries no merge stages — bail to the
        // dialog rather than offer a partial resolution.
        let stages = file.stages.as_ref()?;
        // A whole-file add / delete conflict (a missing ours or
        // theirs stage) is not a per-node content conflict — the
        // node-level view cannot resolve it, so bail to the dialog.
        let (Some(base), Some(ours), Some(theirs)) = (&stages.base, &stages.ours, &stages.theirs)
        else {
            return None;
        };
        let result = op_opmerge::merge_op_documents(base, ours, theirs).ok()?;
        if result.conflicts.is_empty() {
            // Structurally clean already (the auto-apply resolver
            // would have handled it) — nothing to resolve here.
            continue;
        }
        let rows = result
            .conflicts
            .iter()
            .map(|c| MergeConflictRow {
                id: c.id.clone(),
                label: c.label.clone(),
                kind: op_i18n::translate(locale, c.kind.i18n_key()).to_string(),
                theirs_allowed: c.theirs_applicable,
                take_theirs: false,
            })
            .collect();
        let (base, ours, theirs) = (base.clone(), ours.clone(), theirs.clone());
        files.push(MergeResolveFile {
            path: file.path.clone(),
            base,
            ours,
            theirs,
            conflicts: rows,
        });
    }
    if files.is_empty() {
        return None;
    }
    Some(MergeResolveState {
        branch: branch.to_string(),
        files,
    })
}

/// Build a self-contained patch for hunk `hunk_index` of a unified
/// diff — the file header (everything before the first `@@`) plus
/// just that hunk's lines. `None` when the diff has no such hunk.
pub(super) fn build_hunk_patch(lines: &[String], hunk_index: usize) -> Option<String> {
    let first = lines.iter().position(|l| l.starts_with("@@"))?;
    let hunk_starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("@@"))
        .map(|(i, _)| i)
        .collect();
    let &start = hunk_starts.get(hunk_index)?;
    let end = hunk_starts
        .get(hunk_index + 1)
        .copied()
        .unwrap_or(lines.len());
    let mut patch = String::new();
    for line in &lines[..first] {
        patch.push_str(line);
        patch.push('\n');
    }
    for line in &lines[start..end] {
        patch.push_str(line);
        patch.push('\n');
    }
    Some(patch)
}

/// Build the per-file conflict breakdown for the merge dialog. A
/// `.op` file's three index stages are run through the structured
/// node-level merge ([`op_opmerge`]) so the report names the
/// conflicting PenNodes — or flags the file as structurally
/// auto-mergeable when the node merge is clean despite git's
/// line-level conflict. Non-`.op` files list as a bare path.
pub(super) fn merge_conflict_detail(
    conflicts: &op_git::ConflictBag,
    locale: op_editor_core::Locale,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for file in &conflicts.files {
        let Some(stages) = &file.stages else {
            lines.push(file.path.clone());
            continue;
        };
        // A missing stage (e.g. add/add has no base) reads as an
        // empty document for the structured merge.
        let base = stages.base.as_deref().unwrap_or("{}");
        let ours = stages.ours.as_deref().unwrap_or("{}");
        let theirs = stages.theirs.as_deref().unwrap_or("{}");
        match op_opmerge::merge_op_documents(base, ours, theirs) {
            Ok(result) if result.is_clean() => {
                lines.push(
                    op_i18n::translate(locale, "git.merge.autoMergeable")
                        .replace("{{path}}", &file.path),
                );
            }
            Ok(result) => {
                lines.push(format!("{}:", file.path));
                for node in result.conflicts.iter().take(12) {
                    let kind = op_i18n::translate(locale, node.kind.i18n_key());
                    lines.push(format!("    • {} — {}", node.label, kind));
                }
                if result.conflicts.len() > 12 {
                    lines.push(
                        op_i18n::translate(locale, "git.merge.andMore")
                            .replace("{{count}}", &(result.conflicts.len() - 12).to_string()),
                    );
                }
            }
            Err(_) => lines.push(file.path.clone()),
        }
    }
    lines.join("\n")
}
