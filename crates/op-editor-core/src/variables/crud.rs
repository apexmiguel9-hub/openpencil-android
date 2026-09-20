//! Variable table CRUD — create / delete / rename (with document-wide
//! ref rewriting), bulk replacement and the active theme-axis
//! selection.

use super::*;

impl EditorState {
    // --- Variable lifecycle -----------------------------------------

    /// Create a new theme-agnostic scalar variable. Rejects an empty
    /// (post-trim) name, a name that collides with an existing
    /// variable, or a `default` that doesn't match `kind`. Color
    /// defaults must be a parseable hex string.
    pub fn create_variable(
        &mut self,
        name: &str,
        kind: VariableKind,
        default: VariableScalar,
    ) -> bool {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return false;
        }
        let kind_ok = match (&kind, &default) {
            (VariableKind::Color, VariableScalar::Str(s)) => {
                crate::color_picker::parse_hex_rgb(s).is_some()
            }
            (VariableKind::Number, VariableScalar::Num(_)) => true,
            (VariableKind::String, VariableScalar::Str(_)) => true,
            (VariableKind::Boolean, VariableScalar::Bool(_)) => true,
            _ => false,
        };
        if !kind_ok {
            return false;
        }
        let vars = self.variables_mut();
        if vars.contains_key(trimmed) {
            return false;
        }
        vars.insert(
            trimmed.to_string(),
            VariableDefinition {
                kind,
                value: VariableValue::Scalar(default),
            },
        );
        true
    }

    /// Delete a variable by name. `false` when the name is unknown.
    ///
    /// Every `$name` reference in the node tree is replaced by its
    /// resolved concrete value FIRST (TS `removeVariable` →
    /// `replaceVariableRefsInTree(old, null)`), so nodes freeze the
    /// last resolved colour / number instead of keeping a dangling
    /// token that fails to resolve on the next load.
    pub fn delete_variable(&mut self, name: &str) -> bool {
        if !self
            .doc
            .variables
            .as_ref()
            .is_some_and(|vars| vars.contains_key(name))
        {
            return false;
        }
        // Resolve-and-replace while the definition still exists —
        // the concrete replacement value comes from it.
        self.replace_refs_in_doc(name, None);
        let Some(vars) = self.doc.variables.as_mut() else {
            return false;
        };
        if vars.remove(name).is_none() {
            return false;
        }
        // Drop any ref caches pointing at the now-deleted variable.
        self.ui.variables.fill_refs.retain(|_, v| v != name);
        self.ui.variables.stroke_refs.retain(|_, v| v != name);
        true
    }

    /// Rewrite every `$old` token in the document tree to `$new`
    /// (rename) or its resolved concrete value (`None`, delete).
    /// Walks pages AND the single-page fallback children.
    fn replace_refs_in_doc(&mut self, old: &str, new: Option<&str>) {
        let vars_snapshot = self.doc.variables.clone();
        let theme =
            crate::variables_resolve::effective_theme(&self.doc, &self.ui.variables.active_theme);
        if let Some(pages) = self.doc.pages.as_mut() {
            for page in pages {
                crate::variables_resolve::replace_variable_refs_in_tree(
                    &mut page.children,
                    old,
                    new,
                    vars_snapshot.as_ref(),
                    &theme,
                );
            }
        }
        crate::variables_resolve::replace_variable_refs_in_tree(
            &mut self.doc.children,
            old,
            new,
            vars_snapshot.as_ref(),
            &theme,
        );
    }

    /// Rename a variable. Rejects an unknown `old`, an empty (post-
    /// trim) `new`, or a `new` that collides with a different
    /// existing variable (DELIBERATE divergence: TS
    /// `variable-manager.ts` silently overwrites the collision —
    /// refusing is safer and the panel surfaces the failure).
    /// `old == new` (after trim) is a no-op success. Rewrites every
    /// `fill_refs` / `stroke_refs` entry AND every `$old` token in
    /// the node tree.
    pub fn rename_variable(&mut self, old: &str, new: &str) -> bool {
        let new_trimmed = new.trim();
        if new_trimmed.is_empty() {
            return false;
        }
        let Some(vars) = self.doc.variables.as_mut() else {
            return false;
        };
        if old == new_trimmed {
            return vars.contains_key(old);
        }
        if vars.contains_key(new_trimmed) {
            return false;
        }
        let Some(def) = vars.remove(old) else {
            return false;
        };
        vars.insert(new_trimmed.to_string(), def);
        for v in self.ui.variables.fill_refs.values_mut() {
            if v == old {
                *v = new_trimmed.to_string();
            }
        }
        for v in self.ui.variables.stroke_refs.values_mut() {
            if v == old {
                *v = new_trimmed.to_string();
            }
        }
        // Rewrite `$old` → `$new` across the node tree so persisted
        // documents don't keep dangling tokens (TS `renameVariable` →
        // `replaceVariableRefsInTree(old, new)`).
        let new_owned = new_trimmed.to_string();
        self.replace_refs_in_doc(old, Some(&new_owned));
        true
    }

    /// Merge or replace the persisted document variables map. This
    /// matches the TS `set_variables` bulk operation used by the CLI
    /// and MCP route.
    pub fn set_variables_bulk(
        &mut self,
        variables: BTreeMap<String, VariableDefinition>,
        replace: bool,
    ) -> bool {
        if replace {
            self.doc.variables = Some(variables);
        } else {
            self.variables_mut().extend(variables);
        }
        true
    }

    /// Merge or replace the persisted document theme axes. Any
    /// transient active-theme pin that no longer points at a declared
    /// axis/value is dropped so later variable resolution cannot keep
    /// using a stale selection.
    pub fn set_themes_bulk(
        &mut self,
        themes: BTreeMap<String, Vec<String>>,
        replace: bool,
    ) -> bool {
        if replace {
            self.doc.themes = Some(themes);
        } else {
            self.doc
                .themes
                .get_or_insert_with(BTreeMap::new)
                .extend(themes);
        }
        let declared = self.doc.themes.clone().unwrap_or_default();
        self.ui.variables.active_theme.retain(|axis, value| {
            declared
                .get(axis)
                .is_some_and(|values| values.iter().any(|v| v == value))
        });
        true
    }

    // --- Theme axis -------------------------------------------------

    /// Set the active value of a theme axis (e.g. `("mode", "dark")`).
    /// `false` when the axis isn't declared in `doc.themes` or the
    /// value isn't one of its declared entries.
    pub fn set_active_axis_value(&mut self, axis: &str, value: &str) -> bool {
        let Some(themes) = self.doc.themes.as_ref() else {
            return false;
        };
        let Some(values) = themes.get(axis) else {
            return false;
        };
        if !values.iter().any(|v| v == value) {
            return false;
        }
        self.ui
            .variables
            .active_theme
            .insert(axis.to_string(), value.to_string());
        true
    }

    /// Cycle the active value of `axis` to the next declared entry,
    /// wrapping at the end. Seeds the first value when the axis
    /// isn't yet in the active theme. `false` when the axis isn't
    /// declared or has no values.
    pub fn cycle_active_axis_value(&mut self, axis: &str) -> bool {
        let Some(themes) = self.doc.themes.as_ref() else {
            return false;
        };
        let Some(values) = themes.get(axis) else {
            return false;
        };
        if values.is_empty() {
            return false;
        }
        let next = match self.ui.variables.active_theme.get(axis) {
            None => values[0].clone(),
            Some(current) => {
                let idx = values
                    .iter()
                    .position(|v| v == current)
                    .map(|i| (i + 1) % values.len())
                    .unwrap_or(0);
                values[idx].clone()
            }
        };
        self.ui
            .variables
            .active_theme
            .insert(axis.to_string(), next);
        true
    }
}
