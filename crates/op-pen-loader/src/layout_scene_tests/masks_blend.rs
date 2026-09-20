//! Path fill rules, mask markers / source opacity, blend modes,
//! per-corner radii and the Figma image-fill transform.

use super::*;

#[test]
fn layout_scene_carries_even_odd_path_fill_rule() {
    let src = r#"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"path","id":"ring","width":100,"height":100,
         "d":"M0 0H100V100H0Z M25 25H75V75H25Z","fillRule":"evenodd"}
      ]}],"children":[]
    }"#;
    let scene = editor_state_to_layout_scene(&state_from(src));
    assert!(scene.active_page().unwrap().children[0].even_odd_fill);
}

#[test]
fn layout_scene_carries_path_mask_marker() {
    let src = r#"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"path","id":"mask","width":100,"height":100,
         "d":"M0 0H100V100H0Z","mask":true}
      ]}],"children":[]
    }"#;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let mask = &scene.active_page().unwrap().children[0];
    assert!(mask.is_mask);
    assert_eq!(mask.mask_type, Some(MaskType::Alpha));
}

#[test]
fn layout_scene_carries_luminance_mask_on_a_container() {
    let src = r#"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"frame","id":"mask","width":100,"height":100,
         "maskType":"luminance","children":[]}
      ]}],"children":[]
    }"#;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let mask = &scene.active_page().unwrap().children[0];
    assert_eq!(mask.mask_type, Some(MaskType::Luminance));
}

#[test]
fn mask_source_opacity_excludes_the_common_ancestor_opacity() {
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"frame","id":"parent","width":100,"height":100,"opacity":0.5,
         "children":[
           {"type":"rectangle","id":"mask","width":10,"height":10,
            "opacity":0.5,"maskType":"alpha",
            "fill":[{"type":"solid","color":"#ffffff"}]}
         ]}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let parent = &scene.active_page().unwrap().children[0];
    let mask = &parent.children[0];
    assert!((parent.opacity - 1.0).abs() < 0.001);
    assert!((parent.composite_opacity - 0.5).abs() < 0.001);
    assert!((mask.opacity - 0.5).abs() < 0.001);
    assert!((mask.composite_opacity - 1.0).abs() < 0.001);
}

#[test]
fn node_blend_carries_local_opacity_on_the_composite_layer() {
    let src = r#"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"frame","id":"parent","width":100,"height":100,
         "opacity":0.5,"blendMode":"soft_light","children":[
           {"type":"rectangle","id":"child","width":10,"height":10}
         ]}
      ]}],"children":[]
    }"#;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let parent = &scene.active_page().unwrap().children[0];
    assert_eq!(parent.blend_mode, ImageBlendMode::SoftLight);
    assert_eq!(parent.children[0].blend_mode, ImageBlendMode::Normal);
    assert!((parent.opacity - 1.0).abs() < 0.001);
    assert!((parent.composite_opacity - 0.5).abs() < 0.001);
    assert!((parent.children[0].opacity - 1.0).abs() < 0.001);
    assert!((parent.children[0].composite_opacity - 1.0).abs() < 0.001);
}

#[test]
fn extended_canonical_blend_modes_map_to_scene_modes() {
    use jian_ops_schema::style::BlendMode;
    for (canonical, scene) in [
        (BlendMode::SoftLight, ImageBlendMode::SoftLight),
        (BlendMode::ColorDodge, ImageBlendMode::ColorDodge),
        (BlendMode::ColorBurn, ImageBlendMode::ColorBurn),
        (BlendMode::HardLight, ImageBlendMode::HardLight),
        (BlendMode::Exclusion, ImageBlendMode::Exclusion),
    ] {
        assert_eq!(blend_mode_to_scene(Some(&canonical)), scene);
    }
}

#[test]
fn layout_scene_carries_per_corner_radii() {
    let src = r#"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","width":100,"height":50,
         "cornerRadius":[8,0,6,2]}
      ]}],"children":[]
    }"#;
    let scene = editor_state_to_layout_scene(&state_from(src));
    assert_eq!(
        scene.active_page().unwrap().children[0].corner_radii,
        Some([8.0, 0.0, 6.0, 2.0])
    );
}

#[test]
fn layout_scene_carries_figma_image_fill_transform() {
    let src = r#"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"crop","width":375,"height":490,
         "fill":[{"type":"image","url":"data:image/png;base64,AA==","mode":"crop",
          "transform":{"m00":0.9999718,"m01":0.0,"m02":0.00001408411,
                       "m10":0.0,"m11":0.602706,"m12":0.12054121}}]}
      ]}],"children":[]
    }"#;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let crop = &scene.active_page().unwrap().children[0];

    assert_eq!(
        crop.image_transform,
        Some([0.9999718, 0.0, 0.00001408411, 0.0, 0.602706, 0.12054121])
    );
}
