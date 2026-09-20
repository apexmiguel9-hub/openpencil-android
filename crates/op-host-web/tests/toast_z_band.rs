//! Cross-host z-order parity for the transient notice banner.
//!
//! Split out of `paint_order.rs`, which sits at the 800-line cap. Same
//! source-position convention: the hosts' paint and press spines encode the
//! z-band in their call order, so the guard reads that order back.

/// Both hosts must paint the toast banner in the same z-band, and hit-test it
/// in the mirrored position — hit-test runs in reverse paint order, so a
/// surface painted between the diagnostics notice and the missing-font modal
/// must be pressed between the modal and the notice. Drift here is invisible
/// until a user on one platform cannot click the dismiss cross.
#[test]
fn both_hosts_place_the_toast_in_the_same_z_band() {
    let hosts = [
        format!("{}/src", env!("CARGO_MANIFEST_DIR")),
        format!("{}/../op-host-native/src", env!("CARGO_MANIFEST_DIR")),
    ];
    for host in hosts {
        // The band lives in whichever file that host keeps it in: web has it
        // inline in the paint spine, native split it into a sibling at the
        // 800-line cap. Either way it is one contiguous block.
        let paint = [
            "widget_host/paint_topmost_overlays.rs",
            "widget_host/paint.rs",
        ]
        .into_iter()
        .filter_map(|name| std::fs::read_to_string(format!("{host}/{name}")).ok())
        .find(|source| source.contains("editor_toast_flow::paint"))
        .expect("one file carries the top-most overlay band");
        let diagnostics = paint
            .find("HtmlImportDiagnosticsPanel::for_editor")
            .expect("the diagnostics notice paints");
        let toast = paint
            .find("editor_toast_flow::paint")
            .expect("the toast paints");
        let modal = paint
            .find("MissingFontsPanel::for_editor")
            .expect("the missing-font modal paints");
        assert!(
            diagnostics < toast && toast < modal,
            "{host}: the toast belongs above every panel and the diagnostics \
             notice, but under the missing-font modal"
        );

        let press = std::fs::read_to_string(format!("{host}/widget_host/press_overlay_tiers.rs"))
            .expect("press tier source is readable");
        let modal_press = press
            .find("dispatch_missing_fonts_press")
            .expect("the modal hit-tests");
        let toast_press = press
            .find("editor_toast_flow::press")
            .expect("the toast hit-tests");
        let diagnostics_press = press
            .find("dispatch_html_import_diagnostics_press")
            .expect("the diagnostics notice hit-tests");
        assert!(
            modal_press < toast_press && toast_press < diagnostics_press,
            "{host}: hit-test is reverse paint order, so the toast must be \
             pressed after the modal and before the diagnostics notice"
        );
    }
}
