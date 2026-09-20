use std::collections::BTreeMap;

use super::*;
use crate::mapper::MapCtx;
use crate::HtmlImportOptions;

fn style(pairs: &[(&str, &str)]) -> ComputedStyle {
    ComputedStyle {
        props: pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
        font_size: 16.0,
    }
}

fn context(options: &HtmlImportOptions) -> MapCtx<'_> {
    MapCtx {
        opts: options,
        rules: &[],
        warnings: Vec::new(),
        warned: Default::default(),
        next_id: 0,
        node_count: 0,
        containing_width: options.viewport_width,
        containing_height: options.viewport_height(),
        containing_width_is_definite: true,
        positioned_width: options.viewport_width,
        positioned_height: options.viewport_height(),
        auto_margin_handled_by_parent: false,
        pending_base_outcome: Default::default(),
    }
}

fn bake(css: &str, width: f64, height: f64) -> (Option<TransformBake>, Vec<String>) {
    let options = HtmlImportOptions::default();
    let mut context = context(&options);
    let style = style(&[("transform", css)]);
    let bake = bake_transform(&style, &mut context, width, height, (true, true));
    (bake, crate::render_warnings(&context.warnings))
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-6
}

#[test]
fn splits_a_multi_function_list_with_nested_calls() {
    let parsed = split_functions("translate( calc(10px + 2px) , -50% ) rotate(45deg)")
        .expect("well-formed list");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].0, "translate");
    assert_eq!(parsed[0].1, ["calc(10px + 2px)", "-50%"]);
    assert_eq!(parsed[1].0, "rotate");
    assert_eq!(parsed[1].1, ["45deg"]);
    // A dangling name or an unbalanced paren is rejected outright.
    assert!(split_functions("rotate(45deg").is_none());
    assert!(split_functions("rotate").is_none());
}

#[test]
fn parses_every_angle_unit() {
    assert_eq!(parse_angle("90deg"), Some(90.0));
    assert_eq!(parse_angle("0.5turn"), Some(180.0));
    assert_eq!(parse_angle("100grad"), Some(90.0));
    assert!(close(parse_angle("3.14159265rad").unwrap(), 180.0));
    assert_eq!(parse_angle("0"), Some(0.0));
    assert_eq!(parse_angle("12"), None);
}

#[test]
fn composes_functions_in_list_order() {
    // `translate` then `rotate` moves first, so the origin lands at (10, 0)
    // before the rotation spins the box around it.
    let composed = Affine::translate(10.0, 0.0).multiply(Affine::rotate_degrees(90.0));
    assert!(close(composed.e, 10.0));
    assert!(close(composed.a, 0.0));
    assert!(close(composed.b, 1.0));

    let decomposed = decompose(Affine::rotate_degrees(30.0).multiply(Affine::scale(2.0, 3.0)));
    assert!(close(decomposed.rotation_degrees, 30.0));
    assert!(close(decomposed.scale.0, 2.0));
    assert!(close(decomposed.scale.1, 3.0));
    assert!(close(decomposed.shear, 0.0));
}

#[test]
fn decomposes_skew_into_a_reported_shear() {
    let decomposed = decompose(Affine::skew_degrees(45.0, 0.0));
    assert!(close(decomposed.shear, 1.0));
    assert!(close(decomposed.scale.0, 1.0));
    assert!(close(decomposed.rotation_degrees, 0.0));
}

#[test]
fn percentage_translate_resolves_against_the_element_box() {
    let (bake, warnings) = bake("translate(-50%, -50%)", 320.0, 180.0);
    let bake = bake.expect("translate bakes");
    assert!(close(bake.translate.0, -160.0));
    assert!(close(bake.translate.1, -90.0));
    assert!(close(bake.rotation_degrees, 0.0));
    assert_eq!(bake.scale, (1.0, 1.0));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn scale_about_the_default_centre_shifts_the_box_left_and_up() {
    // `scale(2)` about 50% 50% doubles the box and moves its origin by
    // -w/2 / -h/2 so the centre stays put.
    let (bake, _) = bake("scale(2)", 100.0, 40.0);
    let bake = bake.expect("scale bakes");
    assert!(close(bake.scale.0, 2.0));
    assert!(close(bake.scale.1, 2.0));
    assert!(close(bake.translate.0, -50.0));
    assert!(close(bake.translate.1, -20.0));
}

#[test]
fn transform_origin_keywords_move_the_pivot() {
    let options = HtmlImportOptions::default();
    let mut context = context(&options);
    let corner_origin = style(&[("transform", "scale(2)"), ("transform-origin", "top left")]);
    let bake = bake_transform(&corner_origin, &mut context, 100.0, 40.0, (true, true))
        .expect("scale bakes");
    // Scaling about the top-left corner leaves the origin where it was.
    assert!(close(bake.translate.0, 0.0));
    assert!(close(bake.translate.1, 0.0));

    let right_origin = style(&[("transform", "scale(2)"), ("transform-origin", "right 25%")]);
    let bake = bake_transform(&right_origin, &mut context, 100.0, 40.0, (true, true))
        .expect("scale bakes");
    assert!(close(bake.translate.0, -100.0));
    assert!(close(bake.translate.1, -10.0));
}

#[test]
fn matrix_and_multi_function_lists_round_trip_through_the_decomposition() {
    let (matrix, _) = bake("matrix(0, 1, -1, 0, 12, 34)", 0.0, 0.0);
    let matrix = matrix.expect("matrix bakes");
    assert!(close(matrix.rotation_degrees, 90.0));
    assert!(close(matrix.translate.0, 12.0));
    assert!(close(matrix.translate.1, 34.0));

    let (list, _) = bake(
        "translateX(20px) translateY(-8px) rotate(0.25turn)",
        0.0,
        0.0,
    );
    let list = list.expect("list bakes");
    assert!(close(list.translate.0, 20.0));
    assert!(close(list.translate.1, -8.0));
    assert!(close(list.rotation_degrees, 90.0));
}

#[test]
fn skew_and_three_d_functions_warn() {
    let (_, warnings) = bake("skewX(20deg)", 10.0, 10.0);
    assert!(
        warnings.iter().any(|warning| warning.contains("skew")),
        "{warnings:?}"
    );

    let (_, warnings) = bake("perspective(500px) rotateY(30deg)", 10.0, 10.0);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("unsupported CSS transform function")),
        "{warnings:?}"
    );
}

#[test]
fn mirroring_reports_and_keeps_the_absolute_scale() {
    let (bake, warnings) = bake("scaleX(-1)", 100.0, 20.0);
    let bake = bake.expect("mirror bakes");
    assert!(close(bake.scale.0, 1.0));
    assert!(
        warnings.iter().any(|warning| warning.contains("mirroring")),
        "{warnings:?}"
    );
}

#[test]
fn none_and_empty_transforms_are_inert() {
    assert!(bake("none", 10.0, 10.0).0.is_none());
    assert!(bake("   ", 10.0, 10.0).0.is_none());
}

/// C5. Either axis can carry a mirror, and the decomposition has to pick the
/// one that leaves the smaller rotation. Always flipping X turned
/// `scaleY(-1)` into a −180° spin — the node imported upside down.
#[test]
fn a_vertical_mirror_does_not_import_as_a_half_turn() {
    let (vertical, warnings) = bake("scaleY(-1)", 100.0, 20.0);
    let vertical = vertical.expect("mirror bakes");
    assert!(close(vertical.rotation_degrees, 0.0), "{vertical:?}");
    assert!(close(vertical.scale.0, 1.0));
    assert!(close(vertical.scale.1, 1.0));
    assert!(
        warnings.iter().any(|warning| warning.contains("mirroring")),
        "{warnings:?}"
    );

    // The horizontal mirror keeps its own zero rotation too.
    let (horizontal, _) = bake("scaleX(-1)", 100.0, 20.0);
    assert!(close(
        horizontal.expect("mirror bakes").rotation_degrees,
        0.0
    ));

    // A mirror composed with a real rotation still reports that rotation.
    let (rotated, _) = bake("rotate(30deg) scaleY(-1)", 100.0, 20.0);
    assert!(close(rotated.expect("bakes").rotation_degrees, 30.0));
}

/// C6. `transform: scale(0)` is the hidden-popover idiom; the schema cannot
/// store a zero-extent box, so the collapse has to be reported rather than
/// silently normalised back to 1.
#[test]
fn a_collapsing_scale_is_reported_instead_of_silently_normalised() {
    let (collapsed, warnings) = bake("scale(0)", 100.0, 20.0);
    let collapsed = collapsed.expect("scale bakes");
    assert_eq!(
        collapsed.scale,
        (1.0, 1.0),
        "no zero-extent box is representable"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("collapses the element")),
        "{warnings:?}"
    );
    // A degenerate scale is not also reported as mirroring.
    assert!(
        !warnings.iter().any(|warning| warning.contains("mirroring")),
        "{warnings:?}"
    );

    let (_, warnings) = bake("scaleX(0)", 100.0, 20.0);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("collapses the element")),
        "{warnings:?}"
    );
}

/// B3, at the unit level: a percentage translation on an indefinite axis is
/// dropped rather than resolved against the fabricated box.
#[test]
fn percentage_translate_needs_a_definite_axis() {
    let options = HtmlImportOptions::default();
    let mut percent_context = context(&options);
    let percent = style(&[("transform", "translate(-50%, -50%)")]);
    // Width definite, height standing in for an auto axis.
    let bake = bake_transform(&percent, &mut percent_context, 320.0, 900.0, (true, false))
        .expect("translate bakes");
    assert!(close(bake.translate.0, -160.0));
    assert!(close(bake.translate.1, 0.0));
    let percent_warnings = crate::render_warnings(&percent_context.warnings);
    assert!(
        percent_warnings
            .iter()
            .any(|warning| warning.contains("percentage transform translation dropped")),
        "{:?}",
        percent_warnings
    );

    // Pixel translations are unaffected by definiteness.
    let mut pixel_context = context(&options);
    let pixels = style(&[("transform", "translate(-20px, -30px)")]);
    let bake =
        bake_transform(&pixels, &mut pixel_context, 320.0, 900.0, (false, false)).expect("bakes");
    assert!(close(bake.translate.0, -20.0));
    assert!(close(bake.translate.1, -30.0));
    assert!(
        pixel_context.warnings.is_empty(),
        "{:?}",
        pixel_context.warnings
    );
}

/// B4, at the unit level: the pull-back is half of the SCALED box only on the
/// axes the mapper could actually scale.
#[test]
fn translate_for_applied_scale_uses_the_unscaled_half_on_declined_axes() {
    let (scaled, _) = bake("scale(2)", 100.0, 40.0);
    let scaled = scaled.expect("scale bakes");
    assert_eq!(
        scaled.translate_for_applied_scale((true, true)),
        (-50.0, -20.0)
    );
    assert_eq!(
        scaled.translate_for_applied_scale((false, false)),
        (0.0, 0.0)
    );
    assert_eq!(
        scaled.translate_for_applied_scale((true, false)),
        (-50.0, 0.0)
    );
}
