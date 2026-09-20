//! Variable lookup, node colour bindings and the typed scalar setters
//! (colour / number / string / boolean, plain + per-theme).

use super::*;

impl EditorState {
    // --- Read helpers -----------------------------------------------

    /// Mutable handle to `doc.variables`, creating the map on first
    /// use so the mutators never have to special-case `None`.
    pub(super) fn variables_mut(&mut self) -> &mut BTreeMap<String, VariableDefinition> {
        self.doc.variables.get_or_insert_with(BTreeMap::new)
    }

    /// Look up a variable definition by name.
    pub fn find_variable(&self, name: &str) -> Option<&VariableDefinition> {
        self.doc.variables.as_ref()?.get(name)
    }

    /// Resolve a variable's current scalar under the active theme.
    /// `None` for an unknown name or an empty themed list.
    pub fn resolve_variable(&self, name: &str) -> Option<&VariableScalar> {
        let def = self.find_variable(name)?;
        resolve_value(&def.value, &self.ui.variables.active_theme)
    }

    /// Write `$name` into the selected node's fill/stroke colour and
    /// update the transient variable-ref cache used by paint-time
    /// resolution. Returns false when the selection is not editable,
    /// the target is not Fill/Stroke, or `name` is not a Color variable.
    pub fn bind_selected_color_variable(&mut self, target: ColorTarget, name: &str) -> bool {
        let Some(name) = normalize_variable_ref_name(name) else {
            return false;
        };
        let Some(def) = self.find_variable(name) else {
            return false;
        };
        if !matches!(def.kind, VariableKind::Color) {
            return false;
        }

        let sel = self.selection.anchor.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return false;
        }
        let Some(node) = find_node_mut(self.active_children_mut(), &sel) else {
            return false;
        };
        let value = format!("${name}");
        let wrote = match target {
            ColorTarget::Fill => set_primary_fill_hex(node, &value),
            ColorTarget::Stroke => set_primary_stroke_hex(node, &value),
            ColorTarget::GradientStop(_) | ColorTarget::EffectColor(_) => false,
        };
        if !wrote {
            return false;
        }
        match target {
            ColorTarget::Fill => {
                self.ui.variables.fill_refs.insert(sel, name.to_string());
            }
            ColorTarget::Stroke => {
                self.ui.variables.stroke_refs.insert(sel, name.to_string());
            }
            ColorTarget::GradientStop(_) | ColorTarget::EffectColor(_) => {}
        }
        true
    }

    /// Resolve the selected node's fill/stroke variable binding back
    /// into a concrete colour. This mirrors the TS picker `unbind`
    /// path, which writes the resolved active-theme value.
    pub fn unbind_selected_color_variable(&mut self, target: ColorTarget) -> bool {
        let sel = self.selection.anchor.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return false;
        }
        let Some(name) = self
            .selected_color_variable_name(target)
            .map(str::to_string)
        else {
            return false;
        };
        let hex = self
            .resolve_color_variable_hex(&name)
            .unwrap_or_else(|| "#000000".to_string());
        let Some(node) = find_node_mut(self.active_children_mut(), &sel) else {
            return false;
        };
        let wrote = match target {
            ColorTarget::Fill => set_primary_fill_hex(node, &hex),
            ColorTarget::Stroke => set_primary_stroke_hex(node, &hex),
            ColorTarget::GradientStop(_) | ColorTarget::EffectColor(_) => false,
        };
        if !wrote {
            return false;
        }
        match target {
            ColorTarget::Fill => {
                self.ui.variables.fill_refs.remove(&sel);
            }
            ColorTarget::Stroke => {
                self.ui.variables.stroke_refs.remove(&sel);
            }
            ColorTarget::GradientStop(_) | ColorTarget::EffectColor(_) => {}
        }
        true
    }

    /// Current selected node's fill/stroke `$variable` binding name,
    /// if the authored colour field is a variable reference.
    pub fn selected_color_variable_name(&self, target: ColorTarget) -> Option<&str> {
        let node = self.selected_node()?;
        let raw = match target {
            ColorTarget::Fill => first_solid_fill_hex(node),
            ColorTarget::Stroke => first_solid_stroke_hex(node),
            ColorTarget::GradientStop(_) | ColorTarget::EffectColor(_) => None,
        }?;
        raw.strip_prefix('$').filter(|name| !name.is_empty())
    }

    /// Resolve a Color variable to its active-theme hex string.
    pub fn resolve_color_variable_hex(&self, name: &str) -> Option<String> {
        let def = self.find_variable(name)?;
        if !matches!(def.kind, VariableKind::Color) {
            return None;
        }
        match self.resolve_variable(name)? {
            VariableScalar::Str(hex) if crate::color_picker::parse_hex_rgb(hex).is_some() => {
                Some(hex.clone())
            }
            _ => None,
        }
    }

    // --- Scalar writes ----------------------------------------------

    /// Write a `#rgb` / `#rrggbb` / `#rrggbbaa` hex into a `Color`
    /// variable. `false` when the variable is unknown, not
    /// Color-kind, or the hex doesn't parse. Themed variables route
    /// per the active-theme discipline.
    pub fn set_variable_color(&mut self, name: &str, hex: &str) -> bool {
        if crate::color_picker::parse_hex_rgb(hex).is_none() {
            return false;
        }
        let active = self.ui.variables.active_theme.clone();
        let Some(def) = self.variables_mut().get_mut(name) else {
            return false;
        };
        if !matches!(def.kind, VariableKind::Color) {
            return false;
        }
        write_scalar(
            &mut def.value,
            VariableScalar::Str(hex.trim().to_string()),
            &active,
        );
        true
    }

    /// Write a hex into one concrete theme value column of a `Color`
    /// variable. Mirrors TS `variable-row.tsx:93-108 setValueForTheme`
    /// — the clicked variant column gets the new value; a scalar
    /// value materializes the full themed array first. `false` when
    /// the variable is unknown, not Color-kind, or the hex doesn't
    /// parse.
    pub fn set_variable_color_for_theme(
        &mut self,
        name: &str,
        axis: &str,
        theme_value: &str,
        hex: &str,
    ) -> bool {
        if crate::color_picker::parse_hex_rgb(hex).is_none() {
            return false;
        }
        self.set_variable_scalar_for_theme(
            name,
            VariableKind::Color,
            VariableScalar::Str(hex.trim().to_string()),
            axis,
            theme_value,
        )
    }

    /// Write a number into a `Number` variable. Kind-mismatch → false.
    pub fn set_variable_number(&mut self, name: &str, value: f64) -> bool {
        self.set_variable_scalar(name, VariableKind::Number, VariableScalar::Num(value))
    }

    /// Write a number into one concrete theme value column.
    pub fn set_variable_number_for_theme(
        &mut self,
        name: &str,
        axis: &str,
        theme_value: &str,
        value: f64,
    ) -> bool {
        self.set_variable_scalar_for_theme(
            name,
            VariableKind::Number,
            VariableScalar::Num(value),
            axis,
            theme_value,
        )
    }

    /// Write a string into a `String` variable. Kind-mismatch → false.
    pub fn set_variable_string(&mut self, name: &str, value: impl Into<String>) -> bool {
        self.set_variable_scalar(
            name,
            VariableKind::String,
            VariableScalar::Str(value.into()),
        )
    }

    /// Write a string into one concrete theme value column.
    pub fn set_variable_string_for_theme(
        &mut self,
        name: &str,
        axis: &str,
        theme_value: &str,
        value: impl Into<String>,
    ) -> bool {
        self.set_variable_scalar_for_theme(
            name,
            VariableKind::String,
            VariableScalar::Str(value.into()),
            axis,
            theme_value,
        )
    }

    /// Write a boolean into a `Boolean` variable. Kind-mismatch → false.
    pub fn set_variable_boolean(&mut self, name: &str, value: bool) -> bool {
        self.set_variable_scalar(name, VariableKind::Boolean, VariableScalar::Bool(value))
    }

    /// Write a boolean into one concrete theme value column. Rust-only
    /// extension (TS boolean rows are inert) — keeps boolean cells
    /// consistent with the variant-targeted number/string/color writes.
    pub fn set_variable_boolean_for_theme(
        &mut self,
        name: &str,
        axis: &str,
        theme_value: &str,
        value: bool,
    ) -> bool {
        self.set_variable_scalar_for_theme(
            name,
            VariableKind::Boolean,
            VariableScalar::Bool(value),
            axis,
            theme_value,
        )
    }

    /// Shared kind-checked scalar writer for number / string /
    /// boolean. Color variables are FORBIDDEN here — they need hex
    /// validation, which only `set_variable_color` provides.
    fn set_variable_scalar(
        &mut self,
        name: &str,
        expect: VariableKind,
        scalar: VariableScalar,
    ) -> bool {
        let active = self.ui.variables.active_theme.clone();
        let Some(def) = self.variables_mut().get_mut(name) else {
            return false;
        };
        if def.kind != expect {
            return false;
        }
        write_scalar(&mut def.value, scalar, &active);
        true
    }

    fn set_variable_scalar_for_theme(
        &mut self,
        name: &str,
        expect: VariableKind,
        scalar: VariableScalar,
        axis: &str,
        theme_value: &str,
    ) -> bool {
        let Some(theme_values) = self
            .doc
            .themes
            .as_ref()
            .and_then(|themes| themes.get(axis))
            .cloned()
        else {
            return self.set_variable_scalar(name, expect, scalar);
        };
        if !theme_values.iter().any(|value| value == theme_value) {
            return false;
        }
        let Some(def) = self.variables_mut().get_mut(name) else {
            return false;
        };
        if def.kind != expect {
            return false;
        }
        write_scalar_for_theme(&mut def.value, scalar, axis, theme_value, &theme_values);
        true
    }
}
