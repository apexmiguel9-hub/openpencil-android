//! Bounded memoization for fully-resolved component instance children.
//!
//! Figma libraries often repeat the exact same component payload thousands of
//! times. Resolution is deterministic for a component, its override/derived
//! payload, instance size, and the current virtual-GUID assignment epoch. The
//! cache keeps that exact structural key and always deep-clones a hit so the
//! conversion pass can assign independent Pen ids and mutate its local tree.

use super::apply_instance_overrides_cached;
use crate::figma_types::FigVec2;
use crate::kiwi::FigValue;
use crate::tree::TreeNode;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

const DEFAULT_MAX_ENTRIES: usize = 256;
const DEFAULT_MAX_STRUCTURAL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ExactSize {
    x_bits: u64,
    y_bits: u64,
}

impl From<FigVec2> for ExactSize {
    fn from(size: FigVec2) -> Self {
        Self {
            x_bits: size.x.to_bits(),
            y_bits: size.y.to_bits(),
        }
    }
}

#[derive(Clone)]
struct InstanceExpansionKey {
    component_guid: String,
    overrides: Vec<FigValue>,
    derived: Vec<FigValue>,
    size: Option<ExactSize>,
    global_assignment_epoch: u64,
    assignment_epoch: u64,
}

impl InstanceExpansionKey {
    fn new(
        component_guid: &str,
        overrides: Vec<FigValue>,
        derived: Vec<FigValue>,
        size: Option<FigVec2>,
        global_assignment_epoch: u64,
        assignment_epoch: u64,
    ) -> Self {
        Self {
            component_guid: component_guid.to_string(),
            overrides,
            derived,
            size: size.map(ExactSize::from),
            global_assignment_epoch,
            assignment_epoch,
        }
    }

    fn structural_bytes(&self) -> usize {
        self.component_guid.len()
            + self.overrides.iter().map(fig_value_bytes).sum::<usize>()
            + self.derived.iter().map(fig_value_bytes).sum::<usize>()
            + std::mem::size_of::<Option<ExactSize>>()
            + 2 * std::mem::size_of::<u64>()
    }
}

impl PartialEq for InstanceExpansionKey {
    fn eq(&self, other: &Self) -> bool {
        self.component_guid == other.component_guid
            && self.global_assignment_epoch == other.global_assignment_epoch
            && self.assignment_epoch == other.assignment_epoch
            && self.size == other.size
            && fig_values_equal(&self.overrides, &other.overrides)
            && fig_values_equal(&self.derived, &other.derived)
    }
}

impl Eq for InstanceExpansionKey {}

impl Hash for InstanceExpansionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.component_guid.hash(state);
        self.global_assignment_epoch.hash(state);
        self.assignment_epoch.hash(state);
        self.size.hash(state);
        hash_fig_values(&self.overrides, state);
        hash_fig_values(&self.derived, state);
    }
}

fn fig_values_equal(left: &[FigValue], right: &[FigValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| fig_value_equal(left, right))
}

fn fig_value_equal(left: &FigValue, right: &FigValue) -> bool {
    match (left, right) {
        (FigValue::Null, FigValue::Null) => true,
        (FigValue::Bool(a), FigValue::Bool(b)) => a == b,
        (FigValue::Int(a), FigValue::Int(b)) => a == b,
        (FigValue::Uint(a), FigValue::Uint(b)) => a == b,
        (FigValue::Int64(a), FigValue::Int64(b)) => a == b,
        (FigValue::Uint64(a), FigValue::Uint64(b)) => a == b,
        (FigValue::Float(a), FigValue::Float(b)) => a.to_bits() == b.to_bits(),
        (FigValue::Str(a), FigValue::Str(b)) => a == b,
        (FigValue::Bytes(a), FigValue::Bytes(b)) => a == b,
        (FigValue::Array(a), FigValue::Array(b)) => fig_values_equal(a, b),
        (FigValue::Object(a), FigValue::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|((ak, av), (bk, bv))| ak == bk && fig_value_equal(av, bv))
        }
        _ => false,
    }
}

fn hash_fig_values<H: Hasher>(values: &[FigValue], state: &mut H) {
    values.len().hash(state);
    for value in values {
        hash_fig_value(value, state);
    }
}

fn hash_fig_value<H: Hasher>(value: &FigValue, state: &mut H) {
    std::mem::discriminant(value).hash(state);
    match value {
        FigValue::Null => {}
        FigValue::Bool(value) => value.hash(state),
        FigValue::Int(value) => value.hash(state),
        FigValue::Uint(value) => value.hash(state),
        FigValue::Int64(value) => value.hash(state),
        FigValue::Uint64(value) => value.hash(state),
        FigValue::Float(value) => value.to_bits().hash(state),
        FigValue::Str(value) => value.hash(state),
        FigValue::Bytes(value) => value.hash(state),
        FigValue::Array(values) => hash_fig_values(values, state),
        FigValue::Object(pairs) => {
            pairs.len().hash(state);
            for (key, value) in pairs {
                key.hash(state);
                hash_fig_value(value, state);
            }
        }
    }
}

fn fig_value_bytes(value: &FigValue) -> usize {
    std::mem::size_of::<FigValue>()
        + match value {
            FigValue::Str(value) => value.len(),
            FigValue::Bytes(value) => value.len(),
            FigValue::Array(values) => values.iter().map(fig_value_bytes).sum(),
            FigValue::Object(pairs) => pairs
                .iter()
                .map(|(key, value)| key.len() + fig_value_bytes(value))
                .sum(),
            _ => 0,
        }
}

fn tree_structural_bytes(node: &TreeNode) -> usize {
    fig_value_bytes(&node.figma)
        + node
            .children
            .iter()
            .map(tree_structural_bytes)
            .sum::<usize>()
}

struct CachedExpansion {
    children: Vec<TreeNode>,
    structural_bytes: usize,
}

/// Per-import, FIFO-bounded cache. Keys in `insertion_order` are `Rc`-shared
/// with the map, so large override payloads are not duplicated for eviction.
pub(crate) struct InstanceExpansionCache {
    entries: HashMap<Rc<InstanceExpansionKey>, CachedExpansion>,
    insertion_order: VecDeque<Rc<InstanceExpansionKey>>,
    assignment_epochs: HashMap<String, u64>,
    global_assignment_epoch: u64,
    structural_bytes: usize,
    max_entries: usize,
    max_structural_bytes: usize,
    hits: u64,
    misses: u64,
}

impl Default for InstanceExpansionCache {
    fn default() -> Self {
        Self::with_limits(DEFAULT_MAX_ENTRIES, DEFAULT_MAX_STRUCTURAL_BYTES)
    }
}

impl InstanceExpansionCache {
    fn with_limits(max_entries: usize, max_structural_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
            assignment_epochs: HashMap::new(),
            global_assignment_epoch: 0,
            structural_bytes: 0,
            max_entries,
            max_structural_bytes,
            hits: 0,
            misses: 0,
        }
    }

    fn assignment_epoch(&self, component_guid: &str) -> u64 {
        self.assignment_epochs
            .get(component_guid)
            .copied()
            .unwrap_or(0)
    }

    fn bump_assignment_epoch(&mut self, component_guid: &str) -> u64 {
        let epoch = self
            .assignment_epochs
            .entry(component_guid.to_string())
            .or_default();
        *epoch = epoch
            .checked_add(1)
            .expect("instance assignment epoch overflow");
        self.global_assignment_epoch = self
            .global_assignment_epoch
            .checked_add(1)
            .expect("global instance assignment epoch overflow");

        // Outer expansions can contain nested instances whose result depends on
        // this pin. A global fence conservatively invalidates those dependencies.
        self.entries.clear();
        self.insertion_order.clear();
        self.structural_bytes = 0;
        *epoch
    }

    fn get(&mut self, key: &InstanceExpansionKey) -> Option<Vec<TreeNode>> {
        if let Some(entry) = self.entries.get(key) {
            self.hits += 1;
            return Some(entry.children.clone());
        }
        self.misses += 1;
        None
    }

    fn insert(&mut self, key: InstanceExpansionKey, children: &[TreeNode]) {
        if self.max_entries == 0 || self.max_structural_bytes == 0 {
            return;
        }
        let structural_bytes =
            key.structural_bytes() + children.iter().map(tree_structural_bytes).sum::<usize>();
        if structural_bytes > self.max_structural_bytes || self.entries.contains_key(&key) {
            return;
        }
        while !self.entries.is_empty()
            && (self.entries.len() >= self.max_entries
                || self.structural_bytes.saturating_add(structural_bytes)
                    > self.max_structural_bytes)
        {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.structural_bytes =
                    self.structural_bytes.saturating_sub(entry.structural_bytes);
            }
        }
        let key = Rc::new(key);
        self.insertion_order.push_back(Rc::clone(&key));
        self.structural_bytes = self.structural_bytes.saturating_add(structural_bytes);
        self.entries.insert(
            key,
            CachedExpansion {
                children: children.to_vec(),
                structural_bytes,
            },
        );
    }
}

/// Resolve one instance, reusing an exact prior expansion when safe.
pub(crate) fn apply_instance_overrides_memoized(
    cache: &mut InstanceExpansionCache,
    component_guid: &str,
    symbol_node: &TreeNode,
    overrides: Option<Vec<FigValue>>,
    derived: Option<Vec<FigValue>>,
    instance_size: Option<FigVec2>,
    assignment_cache: &mut HashMap<String, String>,
) -> Vec<TreeNode> {
    let mut key = InstanceExpansionKey::new(
        component_guid,
        overrides.unwrap_or_default(),
        derived.unwrap_or_default(),
        instance_size,
        cache.global_assignment_epoch,
        cache.assignment_epoch(component_guid),
    );
    if let Some(children) = cache.get(&key) {
        return children;
    }

    let assignments_before = assignment_cache.len();
    let children = apply_instance_overrides_cached(
        symbol_node,
        Some(&key.overrides),
        Some(&key.derived),
        instance_size,
        assignment_cache,
    );
    // `apply_instance_overrides_cached` inserts only for an unpinned key, so a
    // length increase exactly identifies a newly learned component pin.
    if assignment_cache.len() != assignments_before {
        key.assignment_epoch = cache.bump_assignment_epoch(component_guid);
        key.global_assignment_epoch = cache.global_assignment_epoch;
    }
    cache.insert(key, &children);
    children
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: Vec<(&str, FigValue)>) -> FigValue {
        FigValue::Object(
            pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    fn guid(session_id: u32, local_id: u32) -> FigValue {
        obj(vec![
            ("sessionID", FigValue::Uint(session_id)),
            ("localID", FigValue::Uint(local_id)),
        ])
    }

    fn size(x: f32, y: f32) -> FigValue {
        obj(vec![("x", FigValue::Float(x)), ("y", FigValue::Float(y))])
    }

    fn path(session_id: u32, local_id: u32) -> FigValue {
        obj(vec![(
            "guidPath",
            obj(vec![(
                "guids",
                FigValue::Array(vec![guid(session_id, local_id)]),
            )]),
        )])
    }

    fn override_value(session_id: u32, local_id: u32, name: &str) -> FigValue {
        let mut value = path(session_id, local_id);
        value.set("name", FigValue::Str(name.to_string()));
        value
    }

    fn leaf(name: &str, local_id: u32) -> TreeNode {
        TreeNode {
            figma: obj(vec![
                ("type", FigValue::Str("RECTANGLE".into())),
                ("guid", guid(1, local_id)),
                ("name", FigValue::Str(name.into())),
                ("size", size(100.0, 100.0)),
            ]),
            children: Vec::new(),
        }
    }

    fn symbol() -> TreeNode {
        TreeNode {
            figma: obj(vec![
                ("type", FigValue::Str("SYMBOL".into())),
                ("guid", guid(0, 0)),
                ("size", size(100.0, 100.0)),
            ]),
            children: vec![leaf("child", 10)],
        }
    }

    fn assert_trees_equal(left: &[TreeNode], right: &[TreeNode]) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right) {
            assert_eq!(left.figma, right.figma);
            assert_trees_equal(&left.children, &right.children);
        }
    }

    fn key(component: &str, name: &str, size: Option<FigVec2>, epoch: u64) -> InstanceExpansionKey {
        InstanceExpansionKey::new(
            component,
            vec![override_value(1, 10, name)],
            vec![path(1, 10)],
            size,
            epoch,
            epoch,
        )
    }

    #[test]
    fn identical_signature_hits_and_returns_an_independent_deep_clone() {
        let symbol = symbol();
        let overrides = vec![override_value(1, 10, "renamed")];
        let derived = vec![path(1, 10)];
        let mut assignments = HashMap::new();
        let expected = apply_instance_overrides_cached(
            &symbol,
            Some(&overrides),
            Some(&derived),
            None,
            &mut HashMap::new(),
        );
        let mut cache = InstanceExpansionCache::default();
        let mut first = apply_instance_overrides_memoized(
            &mut cache,
            "0:0",
            &symbol,
            Some(overrides.clone()),
            Some(derived.clone()),
            None,
            &mut assignments,
        );
        assert_trees_equal(&first, &expected);
        first[0].figma.set("name", FigValue::Str("mutated".into()));
        let second = apply_instance_overrides_memoized(
            &mut cache,
            "0:0",
            &symbol,
            Some(overrides),
            Some(derived),
            None,
            &mut assignments,
        );
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
        assert_eq!(second[0].figma.get_str("name"), Some("renamed"));
        assert_trees_equal(&second, &expected);
    }

    #[test]
    fn every_structural_key_dimension_causes_a_miss() {
        let mut cache = InstanceExpansionCache::with_limits(16, 1_000_000);
        let base = key("0:0", "base", Some(FigVec2 { x: 100.0, y: 100.0 }), 0);
        cache.insert(base.clone(), &[leaf("cached", 20)]);
        let mut variants = vec![
            key("0:1", "base", Some(FigVec2 { x: 100.0, y: 100.0 }), 0),
            key("0:0", "override", Some(FigVec2 { x: 100.0, y: 100.0 }), 0),
            key("0:0", "base", Some(FigVec2 { x: 101.0, y: 100.0 }), 0),
            key("0:0", "base", Some(FigVec2 { x: 100.0, y: 100.0 }), 1),
        ];
        let mut changed_derived = base.clone();
        changed_derived.derived.push(path(1, 11));
        variants.push(changed_derived);
        let mut changed_global_epoch = base.clone();
        changed_global_epoch.global_assignment_epoch = 1;
        variants.push(changed_global_epoch);
        for variant in variants {
            assert!(cache.get(&variant).is_none());
        }
        assert!(cache.get(&base).is_some());
    }

    #[test]
    fn new_assignment_epoch_invalidates_nested_dependency_entries() {
        let mut cache = InstanceExpansionCache::with_limits(16, 1_000_000);
        let old = key("0:0", "base", None, 0);
        let other = key("0:1", "other", None, 0);
        cache.insert(old.clone(), &[leaf("old", 20)]);
        cache.insert(other.clone(), &[leaf("other", 21)]);
        assert_eq!(cache.bump_assignment_epoch("0:0"), 1);
        assert!(cache.get(&old).is_none());
        assert!(cache.get(&key("0:0", "base", None, 1)).is_none());
        assert!(cache.get(&other).is_none());
    }

    #[test]
    fn fifo_capacity_evicts_the_oldest_exact_key() {
        let mut cache = InstanceExpansionCache::with_limits(2, 1_000_000);
        let first = key("0:0", "first", None, 0);
        let second = key("0:0", "second", None, 0);
        let third = key("0:0", "third", None, 0);
        cache.insert(first.clone(), &[leaf("first", 20)]);
        cache.insert(second.clone(), &[leaf("second", 21)]);
        cache.insert(third.clone(), &[leaf("third", 22)]);
        assert_eq!(cache.entries.len(), 2);
        assert!(cache.get(&first).is_none());
        assert!(cache.get(&second).is_some());
        assert!(cache.get(&third).is_some());
    }
}
