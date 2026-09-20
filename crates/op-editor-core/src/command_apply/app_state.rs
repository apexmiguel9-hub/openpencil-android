//! [`EditorCommand::MergeAppState`] apply — the additive document
//! `state` merge with its `plan_idx` ownership rules.

use super::*;

impl EditorState {
    /// Apply [`EditorCommand::MergeAppState`]. Backward-compat: never
    /// overwrites a key that already lived in the document root before
    /// this run; among generation-added keys the lower `plan_idx` wins.
    ///
    /// Order-independence is achieved via `self.app_state_owner`: a
    /// side map of `key → owning_plan_idx` for every key written during
    /// this session. On a new key the owner is recorded and the value is
    /// inserted. On a conflicting key the incoming `plan_idx` is compared
    /// to the registered owner; if it is strictly lower it replaces both
    /// the owner record and the document value.
    ///
    /// ## Return contract
    ///
    /// The return value signals **"command processed"**, not **"keys
    /// landed"**. `MergeAppState` is additive by design: doc-owned keys
    /// always win, and among generation-added keys the lower `plan_idx`
    /// wins. A run where every incoming key was skipped (already
    /// doc-owned, or lost the `plan_idx` ownership race) is the designed
    /// steady-state outcome, not a failure — it MUST return `true`.
    ///
    /// This matters beyond the local call site: `MergeAppState` rides
    /// inside `EditorCommand::Batch` alongside a node insert/replace on
    /// every generation path (`hoist_generation_state` +
    /// `with_hoisted_state` in `op-mcp`), and `Batch`'s apply loop
    /// (`command_batch.rs::cmd_batch`) treats the first sub-command that
    /// returns `false` as a hard failure and rolls the ENTIRE batch back.
    /// Returning `false` for a legitimate no-op merge would silently
    /// reject an otherwise-valid insert/replace every time a regenerated
    /// section declares a state key the document root already carries —
    /// a completely normal flow, not a collision. There is currently no
    /// invalid-command shape for `MergeAppState` (any `plan_idx` /
    /// `StateEntry` payload is well-formed), so every path below returns
    /// `true`.
    pub(super) fn merge_app_state(
        &mut self,
        plan_idx: usize,
        incoming: std::collections::BTreeMap<String, jian_ops_schema::state::StateEntry>,
    ) -> bool {
        if incoming.is_empty() {
            // Nothing to merge is a no-op, not a failure — see the
            // return-contract note above. Kept as an early return
            // (rather than falling into the loop) purely to skip the
            // `get_or_insert_with` allocation on doc.state when there is
            // nothing to write into it.
            return true;
        }
        let root = self
            .doc
            .state
            .get_or_insert_with(std::collections::BTreeMap::new);
        for (key, entry) in incoming {
            match self.app_state_owner.entry(key.clone()) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    // Pre-existing doc-root key: owned by the file, skip.
                    // Not a failure — the file's value is authoritative
                    // and is left untouched.
                    if root.contains_key(&key) {
                        continue;
                    }
                    root.insert(key, entry);
                    slot.insert(plan_idx);
                }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    // Generation-added key: lower plan_idx wins. Losing
                    // the race is not a failure — the earlier subtask's
                    // value already won and stays in place.
                    if plan_idx < *slot.get() {
                        tracing::warn!(
                            target: "op.skills",
                            key = %key,
                            winning_plan_idx = plan_idx,
                            losing_plan_idx = *slot.get(),
                            "MergeAppState key conflict — lower plan_idx wins"
                        );
                        root.insert(key, entry);
                        slot.insert(plan_idx);
                    }
                }
            }
        }
        true
    }
}
