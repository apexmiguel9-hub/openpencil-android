//! Structured tally of what the deterministic quality passes checked and
//! repaired, so a finished generation can hand the user a truthful
//! "checked layout, overflow, hierarchy — 6 auto-repairs applied" credential
//! instead of leaving every repair buried in a `tracing` line nobody reads.
//!
//! **What one repair unit means.** Exactly one ACCEPTED `DocSink::apply` —
//! i.e. one document edit a quality pass issued and the sink took. Nothing
//! here interprets intent, guesses at "how many problems that was", or reads
//! pass names: the number is a raw count of edits the passes made, which is
//! the only quantity every pass produces uniformly (most of them return `()`
//! or a bare `bool`). The user-facing wording must stay matched to that
//! meaning — see `op_host_services::quality_credential`.
//!
//! **How the count is taken.** [`RepairCounter::wrap`] returns a
//! [`CountingSink`] that delegates every method to the real sink and records
//! each accepted apply. The cleanup driver shadows its own `sink` binding
//! with that wrapper once, at the top, then calls
//! [`RepairCounter::checkpoint`] between contiguous groups of passes to
//! attribute the edits since the previous checkpoint to a [`CheckCategory`]
//! and a pass-group name. Pass bodies are untouched — this is deliberately a
//! measurement layer, not a refactor of ~40 repair passes.
//!
//! **One list, not two accounts.** Every accepted apply produces exactly one
//! [`RepairRecord`] (`crate::repair_record::describe_command` reads the
//! target node BEFORE the edit so the record carries `before → after`), and
//! the per-category counts this summary reports are that list grouped by
//! category. There is no separately-maintained tally that could drift from
//! the itemized detail the user is shown.
//!
//! **Honesty rules baked in.** A category is recorded as *checked* by the
//! checkpoint itself, whether or not it repaired anything, because reaching
//! the checkpoint proves the detectors ran. A category nothing ever
//! checkpointed never appears. An empty [`RepairSummary`] (no checkpoint at
//! all — e.g. a plain chat turn that never touched the document) must render
//! as no credential at all rather than as "0 problems found".
//!
//! Semantic passes that rewrite the section forest in place (`role_infer` /
//! `role_post_pass` / `tree_heuristics`) never go through a `DocSink`, so
//! their edits are NOT counted; the categories they influence are still
//! covered by the sink-driven passes in `cleanup::run_cleanup_passes`
//! (`repair_overbold_text_hierarchy` for hierarchy, the geometry loop for
//! overflow, and so on).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use jian_ops_schema::node::PenNode;
use op_editor_core::{EditorCommand, EditorState, NodeId};

use crate::repair_record::{describe_command, PendingRepair, RepairRecord};
use crate::types::DocSink;

/// A family of quality checks, as named to the user. Declaration order is
/// display order (and the `Ord` derive that keeps [`RepairSummary`]'s maps
/// deterministic), chosen so the most legible families lead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckCategory {
    /// Sizing / spacing / distribution repairs — padding ownership, footer
    /// sinking, card-height equalization, table column gaps, root height.
    Layout,
    /// The geometry-validation loop: real resolved rects proving text or a
    /// frame overflows its container, a rail collapsed, a row is overfull.
    Overflow,
    /// Visual hierarchy repairs — over-bold text demotion, decorative stroke
    /// stripping.
    Hierarchy,
    /// Structural repairs — duplicate status bars / bottom navs, app-shell
    /// reshaping, flat table rows, empty decorated stubs, broken rings,
    /// cross-screen chrome unification.
    Structure,
    /// Color/theme repairs — split theme-polarity variables, light-mobile nav
    /// surfaces.
    Palette,
}

impl CheckCategory {
    /// Every category, in display order.
    pub const ALL: [CheckCategory; 5] = [
        CheckCategory::Layout,
        CheckCategory::Overflow,
        CheckCategory::Hierarchy,
        CheckCategory::Structure,
        CheckCategory::Palette,
    ];

    /// Stable wire/display key. Also the token the host serializes across the
    /// tool-channel ack, so it must not drift.
    pub fn key(self) -> &'static str {
        match self {
            CheckCategory::Layout => "layout",
            CheckCategory::Overflow => "overflow",
            CheckCategory::Hierarchy => "hierarchy",
            CheckCategory::Structure => "structure",
            CheckCategory::Palette => "palette",
        }
    }

    /// Inverse of [`Self::key`] — used when a summary comes back over a wire
    /// as strings. Unknown keys yield `None` rather than a guess.
    pub fn from_key(key: &str) -> Option<Self> {
        CheckCategory::ALL.into_iter().find(|c| c.key() == key)
    }
}

/// What the quality passes checked and every edit they applied.
///
/// The itemized [`records`](Self::records) list is the single source: the
/// per-category counts every accessor below reports are derived from it by
/// grouping, so the number in the credential and the lines under it can
/// never disagree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepairSummary {
    /// Categories whose passes reached a checkpoint — including those that
    /// found nothing to fix, which is what lets the credential say "checked"
    /// truthfully about a clean family.
    checked: BTreeSet<CheckCategory>,
    /// Every applied edit, in the order the passes applied it.
    records: Vec<RepairRecord>,
    /// Statements about the run that are NOT edits — today, the one line
    /// saying a whole tier of passes was deliberately not run (see
    /// `crate::repair_tier`). Kept apart from `records` on purpose: a record
    /// means "one accepted apply", and a note that counted as one would make
    /// the credential claim a repair that never happened.
    notes: Vec<String>,
}

impl RepairSummary {
    /// Mark `category` as checked and file `records` under it.
    pub fn record_all(&mut self, category: CheckCategory, records: Vec<RepairRecord>) {
        self.checked.insert(category);
        self.records.extend(records);
    }

    /// Count-only ingestion for callers that have a number but no itemized
    /// detail — a wire round-trip from an older host, or a test building a
    /// summary by hand. Synthesizes placeholder records so counts stay
    /// derived from the one list; the placeholders say plainly that no
    /// detail was reported rather than pretending to describe an edit.
    pub fn record(&mut self, category: CheckCategory, repairs: usize) {
        let records = (0..repairs)
            .map(|_| RepairRecord {
                pass: "unattributed".to_string(),
                category,
                node_id: String::new(),
                node_name: None,
                detail: "no detail reported".to_string(),
            })
            .collect();
        self.record_all(category, records);
    }

    /// Add a statement about the run that is not an edit. Repeats are dropped
    /// so a driver that reaches the same gate on several roots still reports
    /// the decision once.
    pub fn note(&mut self, note: impl Into<String>) {
        let note = note.into();
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }

    /// Statements about the run that are not edits, in the order recorded.
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Fold another summary in — used when one run drives cleanup over
    /// several root batches (the orchestrator's append path).
    pub fn merge(&mut self, other: &RepairSummary) {
        self.checked.extend(other.checked.iter().copied());
        self.records.extend(other.records.iter().cloned());
        for note in &other.notes {
            self.note(note.clone());
        }
    }

    /// True when no category was ever checked: render NO credential rather
    /// than claiming a clean bill of health nobody verified.
    pub fn is_empty(&self) -> bool {
        self.checked.is_empty()
    }

    /// Categories whose passes actually ran, in display order.
    pub fn checked(&self) -> Vec<CheckCategory> {
        self.checked.iter().copied().collect()
    }

    /// Every applied edit, in application order.
    pub fn records(&self) -> &[RepairRecord] {
        &self.records
    }

    /// Repairs applied under `category` (0 when checked-and-clean or absent).
    pub fn repairs_for(&self, category: CheckCategory) -> usize {
        self.records
            .iter()
            .filter(|record| record.category == category)
            .count()
    }

    /// Categories that repaired something, paired with their counts, in
    /// display order. Clean categories are omitted.
    pub fn repaired(&self) -> Vec<(CheckCategory, usize)> {
        let mut counts: BTreeMap<CheckCategory, usize> = BTreeMap::new();
        for record in &self.records {
            *counts.entry(record.category).or_insert(0) += 1;
        }
        counts.into_iter().collect()
    }

    /// Total edits applied across every category.
    pub fn total_repairs(&self) -> usize {
        self.records.len()
    }
}

/// Shared edit buffer behind a [`CountingSink`]. Held by the cleanup driver
/// alongside — never inside — the wrapped sink, so a checkpoint can drain the
/// buffer while the sink is still mutably borrowed.
#[derive(Debug)]
pub struct RepairCounter {
    pending: Arc<Mutex<Vec<PendingRepair>>>,
}

impl Default for RepairCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl RepairCounter {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Wrap `inner` so its accepted applies land in this counter.
    pub fn wrap<'a>(&self, inner: &'a mut dyn DocSink) -> CountingSink<'a> {
        CountingSink {
            inner,
            pending: self.pending.clone(),
        }
    }

    /// Attribute every edit since the previous checkpoint to `category` and
    /// the pass group named by `pass`, and mark `category` as checked even
    /// when nothing was applied.
    ///
    /// `pass` names the group of passes that ran since the last checkpoint —
    /// exact for a single-pass group, a family name for a multi-pass one.
    /// Each attributed record is also logged at INFO so a user who pastes
    /// their log can be told which pass touched which node.
    pub fn checkpoint(&mut self, summary: &mut RepairSummary, category: CheckCategory, pass: &str) {
        let drained: Vec<PendingRepair> = match self.pending.lock() {
            Ok(mut pending) => std::mem::take(&mut *pending),
            // A poisoned buffer means a pass panicked mid-apply; the tally is
            // then unknowable, so record a checked-but-empty category rather
            // than a number nobody can stand behind.
            Err(_) => Vec::new(),
        };
        let records: Vec<RepairRecord> = drained
            .into_iter()
            .map(|repair| repair.attribute(category, pass))
            .collect();
        for record in &records {
            tracing::info!(
                pass = %record.pass,
                category = %record.category.key(),
                node = %record.node_id,
                node_name = %record.node_name.as_deref().unwrap_or(""),
                detail = %record.detail,
                "quality repair applied"
            );
        }
        summary.record_all(category, records);
    }
}

/// [`DocSink`] decorator that records accepted applies. Every method
/// delegates verbatim — including `insert_subtree_returning_root_ids`, which
/// MUST forward rather than fall through to the trait default, or an
/// immediate-apply sink's real remapped ids would be swallowed.
///
/// The description is computed BEFORE delegating, against the pre-edit
/// document — that is the only moment the `before` half of `before → after`
/// still exists — and is kept only when the sink accepts the command.
pub struct CountingSink<'a> {
    inner: &'a mut dyn DocSink,
    pending: Arc<Mutex<Vec<PendingRepair>>>,
}

impl CountingSink<'_> {
    fn push(&self, repair: PendingRepair) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.push(repair);
        }
    }
}

impl DocSink for CountingSink<'_> {
    fn state(&self) -> &EditorState {
        self.inner.state()
    }

    fn apply(&mut self, cmd: EditorCommand) -> bool {
        let described = describe_command(self.inner.state(), &cmd);
        let accepted = self.inner.apply(cmd);
        if accepted {
            self.push(described);
        }
        accepted
    }

    fn insert_subtree_returning_root_ids(
        &mut self,
        nodes: Vec<PenNode>,
        parent_id: &NodeId,
    ) -> Option<Vec<String>> {
        let described = describe_command(
            self.inner.state(),
            &EditorCommand::InsertSubtree {
                nodes: Vec::new(),
                parent_id: parent_id.clone(),
                page_id: None,
            },
        );
        let count = nodes.len();
        let out = self
            .inner
            .insert_subtree_returning_root_ids(nodes, parent_id);
        if out.is_some() {
            self.push(PendingRepair {
                detail: format!("inserted {count} subtree(s)"),
                ..described
            });
        }
        out
    }

    fn begin_undo_batch(&mut self) {
        self.inner.begin_undo_batch();
    }

    fn end_undo_batch(&mut self) {
        self.inner.end_undo_batch();
    }
}

#[cfg(test)]
#[path = "repair_summary_tests.rs"]
mod tests;
