use super::*;
use jian_ops_schema::PenDocument;

/// A root of `width x height` whose children are described by `fills`: each
/// entry is `(id, w, h, sksl_len, uniforms_json)`. `uniforms_json` of `null`
/// means the fill declares none.
fn doc_with_shaders(
    root_w: f32,
    root_h: f32,
    fills: &[(&str, f32, f32, usize, serde_json::Value)],
) -> PenNode {
    let children: Vec<String> = fills
        .iter()
        .map(|(id, w, h, len, uniforms)| {
            let sksl = "x".repeat(*len);
            let uniforms = if uniforms.is_null() {
                String::new()
            } else {
                format!(r#","uniforms":{uniforms}"#)
            };
            format!(
                r##"{{ "type": "frame", "id": "{id}", "width": {w}, "height": {h},
                       "fill": [{{ "type": "shader", "sksl": "{sksl}"{uniforms} }}] }}"##
            )
        })
        .collect();
    let src = format!(
        r##"{{ "version": "1.0", "children": [
            {{ "type": "frame", "id": "root", "width": {root_w}, "height": {root_h},
               "layout": "vertical", "children": [{}] }}
        ] }}"##,
        children.join(",")
    );
    let doc: PenDocument = serde_json::from_str(&src).expect("fixture doc");
    doc.children.into_iter().next().expect("root")
}

fn ids(issues: &[Issue]) -> Vec<&str> {
    issues.iter().map(|i| i.node_id.as_str()).collect()
}

#[test]
fn an_ordinary_shader_hero_is_not_flagged() {
    // One full-bleed shader is exactly what a hero section is for.
    let root = doc_with_shaders(
        390.0,
        844.0,
        &[("hero", 390.0, 600.0, 400, serde_json::json!({"t": 1.0}))],
    );
    assert!(
        detect_shader_budget(&root, DesignForm::MobileScreen).is_empty(),
        "a single authored hero shader must pass"
    );
}

#[test]
fn a_phone_tiled_with_full_bleed_passes_is_flagged_past_its_budget() {
    let fills: Vec<_> = (0..5)
        .map(|i| {
            (
                ["a", "b", "c", "d", "e"][i],
                390.0,
                700.0,
                200,
                serde_json::Value::Null,
            )
        })
        .collect();
    let root = doc_with_shaders(390.0, 844.0, &fills);
    let issues = detect_shader_budget(&root, DesignForm::MobileScreen);

    // Budget is 2 on mobile, so passes 3..5 are reported — not all five.
    assert_eq!(ids(&issues), vec!["c", "d", "e"]);
    assert!(issues.iter().all(|i| i.severity == IssueSeverity::Info));
    assert!(
        issues.iter().all(|i| i.suggested_value.is_null()),
        "dropping a visual effect is a design decision, never an auto-fix"
    );
}

#[test]
fn the_same_document_passes_on_a_desktop_page() {
    // Identical content, looser form: the budget is about GPU headroom, and
    // a desktop page has it. Guards against the budget being a blanket rule.
    let fills: Vec<_> = (0..4)
        .map(|i| {
            (
                ["a", "b", "c", "d"][i],
                1440.0,
                700.0,
                200,
                serde_json::Value::Null,
            )
        })
        .collect();
    let root = doc_with_shaders(1440.0, 900.0, &fills);
    assert!(detect_shader_budget(&root, DesignForm::Page).is_empty());
}

#[test]
fn small_shader_accents_do_not_count_against_the_full_bleed_budget() {
    // Eight small shader chips are cheap; the budget is about fragment passes
    // over the whole surface, not about the word "shader" appearing often.
    let fills: Vec<_> = (0..8)
        .map(|i| {
            (
                ["a", "b", "c", "d", "e", "f", "g", "h"][i],
                40.0,
                40.0,
                120,
                serde_json::Value::Null,
            )
        })
        .collect();
    let root = doc_with_shaders(390.0, 844.0, &fills);
    assert!(detect_shader_budget(&root, DesignForm::MobileScreen).is_empty());
}

#[test]
fn a_pasted_shader_toy_is_flagged_on_source_size() {
    let root = doc_with_shaders(
        390.0,
        844.0,
        &[("hero", 100.0, 100.0, 9_000, serde_json::Value::Null)],
    );
    let issues = detect_shader_budget(&root, DesignForm::MobileScreen);
    assert_eq!(issues.len(), 1);
    assert!(
        issues[0].reason.contains("9000 characters"),
        "{:?}",
        issues[0]
    );
}

#[test]
fn a_bad_vec_arity_is_diagnosed_instead_of_silently_degrading() {
    // RuntimeShaderBuilder rejects this at paint time and the fill falls back
    // to a solid colour — which looks like a design choice unless something
    // says otherwise.
    let root = doc_with_shaders(
        390.0,
        844.0,
        &[(
            "hero",
            100.0,
            100.0,
            120,
            serde_json::json!({"tint": [1.0, 0.0, 0.0, 1.0, 0.5]}),
        )],
    );
    let issues = detect_shader_budget(&root, DesignForm::MobileScreen);
    assert_eq!(issues.len(), 1);
    assert!(
        issues[0].reason.contains("`tint` has 5 components"),
        "{:?}",
        issues[0]
    );
    assert!(issues[0].reason.contains("degrade"), "{:?}", issues[0]);
}

#[test]
fn valid_vec_arities_pass() {
    for arity in [2usize, 3, 4] {
        let components: Vec<f32> = vec![0.5; arity];
        let root = doc_with_shaders(
            390.0,
            844.0,
            &[(
                "hero",
                100.0,
                100.0,
                120,
                serde_json::json!({ "v": components }),
            )],
        );
        assert!(
            detect_shader_budget(&root, DesignForm::MobileScreen).is_empty(),
            "vec{arity} is valid SkSL"
        );
    }
}

#[test]
fn an_over_parameterised_shader_is_flagged_on_uniform_count() {
    let mut uniforms = serde_json::Map::new();
    for i in 0..20 {
        uniforms.insert(format!("u{i}"), serde_json::json!(1.0));
    }
    let root = doc_with_shaders(
        390.0,
        844.0,
        &[(
            "hero",
            100.0,
            100.0,
            120,
            serde_json::Value::Object(uniforms),
        )],
    );
    let issues = detect_shader_budget(&root, DesignForm::MobileScreen);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].reason.contains("20 uniforms"), "{:?}", issues[0]);
}

#[test]
fn a_document_with_no_shaders_costs_nothing() {
    let doc: PenDocument = serde_json::from_str(
        r##"{ "version": "1.0", "children": [
            { "type": "frame", "id": "root", "width": 390, "height": 844,
              "fill": [{ "type": "solid", "color": "#101010" }] }
        ] }"##,
    )
    .expect("doc");
    let root = doc.children.into_iter().next().expect("root");
    assert!(detect_shader_budget(&root, DesignForm::MobileScreen).is_empty());
}

/// The split between "cannot render as authored" and "renders but costs a lot"
/// is the whole point of this detector having two categories, and callers gate
/// on severity — `detect_and_plan` drops `Info` outright. Without this test the
/// distinction is invisible to the suite and would be lost in a refactor.
#[test]
fn a_fault_is_a_warning_while_a_cost_stays_advisory() {
    // Bad uniform arity: the fill degrades to a flat colour, so what ships is
    // not the authored design.
    let faulty = doc_with_shaders(
        390.0,
        844.0,
        &[(
            "hero",
            100.0,
            100.0,
            120,
            serde_json::json!({"tint": [1.0, 0.0, 0.0, 1.0, 0.5]}),
        )],
    );
    let issues = detect_shader_budget(&faulty, DesignForm::MobileScreen);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].category, IssueCategory::ShaderInvalid);
    assert_eq!(
        issues[0].severity,
        IssueSeverity::Warning,
        "a fill that cannot render as authored is a defect, not a suggestion"
    );

    // Too many full-bleed passes: expensive, but it renders exactly as asked.
    let costly: Vec<_> = (0..5)
        .map(|i| {
            (
                ["a", "b", "c", "d", "e"][i],
                390.0,
                700.0,
                200,
                serde_json::Value::Null,
            )
        })
        .collect();
    let costly = doc_with_shaders(390.0, 844.0, &costly);
    let issues = detect_shader_budget(&costly, DesignForm::MobileScreen);
    assert!(!issues.is_empty());
    assert!(
        issues
            .iter()
            .all(|i| i.category == IssueCategory::ShaderBudget
                && i.severity == IssueSeverity::Info),
        "dropping a visual effect is a design decision — cost stays advisory"
    );
}

/// Neither kind offers an auto-fix. `apply_fixes` would otherwise try to write
/// `suggested_value` onto the node, and there is no safe machine edit for
/// either "too expensive" or "wrong uniform shape".
#[test]
fn neither_kind_proposes_an_automatic_fix() {
    let faulty = doc_with_shaders(
        390.0,
        844.0,
        &[("hero", 100.0, 100.0, 9_000, serde_json::Value::Null)],
    );
    for issue in detect_shader_budget(&faulty, DesignForm::MobileScreen) {
        assert!(issue.suggested_value.is_null(), "{issue:?}");
    }
}
