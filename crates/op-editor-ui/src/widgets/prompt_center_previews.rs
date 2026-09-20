//! Prompt Center card previews.
//!
//! ~2.0 MB of JPEG across 57 cards. The desktop binary embeds them: it is
//! already a local file, and card painting should never touch the network.
//! The browser bundle does not — 2 MB of already-compressed JPEG is 2 MB the
//! user downloads before the editor paints anything, and the Prompt Center is
//! a panel most sessions never open. On `wasm32` each preview is fetched from
//! the daemon the first time its card paints (see
//! `op_editor_core::web_assets`), and the card shows its text fallback until
//! the bytes land.
//!
//! Both platforms resolve through the same [`prompt_center_preview`] so the
//! paint site has one shape, and the cache id is known on both before any
//! bytes exist — the id is what the renderer's raster cache is keyed on, so it
//! must not depend on whether a fetch has completed.

const PREVIEW_IMAGE_ID_BASE: u64 = 0x5052_4d50_0000_0000;

// Test-only: `concat!` cannot interpolate a const, so the route literals in
// the macro below spell the directory out. This is the value the route
// tests rebuild the expected path from, which is what keeps the spelled-out
// literal and the staged bundle layout (`tools/stage-web-assets.sh`) from
// drifting into a silent per-asset 404.
#[cfg(test)]
const PREVIEW_DIR: &str = "prompt_center_previews";

/// One card's preview, resolved for the current platform.
pub(crate) struct PromptPreview {
    /// Stable renderer cache id. Known before any bytes are, on both hosts.
    pub image_id: u64,
    /// `None` on wasm until the fetch lands; always `Some` on native.
    pub bytes: Option<&'static [u8]>,
    /// Daemon route to fetch. Unused on native, where `bytes` is already set.
    pub route: &'static str,
}

macro_rules! preview {
    ($index:literal, $file:literal) => {{
        // The route literal must agree with `WEB_ASSET_ROUTE_PREFIX`; `concat!`
        // needs literals, so the prefix is spelled out here and checked against
        // the constant by `route_prefix_matches_the_shared_constant` below.
        const ROUTE: &str = concat!("/pkg/assets/", "prompt_center_previews", "/", $file, ".jpg");
        #[cfg(not(target_arch = "wasm32"))]
        let bytes = Some(
            include_bytes!(concat!(
                "../../assets/prompt_center_previews/",
                $file,
                ".jpg"
            ))
            .as_slice(),
        );
        #[cfg(target_arch = "wasm32")]
        let bytes = op_editor_core::web_assets::installed_bytes(ROUTE);
        Some(PromptPreview {
            image_id: PREVIEW_IMAGE_ID_BASE | $index,
            bytes,
            route: ROUTE,
        })
    }};
}

/// Return the preview for a built-in prompt.
///
/// User-defined prompts and unknown ids intentionally have no generated preview
/// and therefore return `None`. A `Some` whose `bytes` are `None` is the web
/// host's "not fetched yet" — a different answer, and the caller must not
/// confuse the two: one means there is no picture, the other means not yet.
pub(crate) fn prompt_center_preview(prompt_id: &str) -> Option<PromptPreview> {
    match prompt_id {
        "gallery-wander" => preview!(1, "gallery-wander"),
        "gallery-forage" => preview!(2, "gallery-forage"),
        "gallery-still" => preview!(3, "gallery-still"),
        "gallery-hearth" => preview!(4, "gallery-hearth"),
        "gallery-meteo" => preview!(5, "gallery-meteo"),
        "gallery-marginalia" => preview!(6, "gallery-marginalia"),
        "gallery-lingua" => preview!(7, "gallery-lingua"),
        "gallery-daybreak" => preview!(8, "gallery-daybreak"),
        "gallery-verdant" => preview!(9, "gallery-verdant"),
        "gallery-companion" => preview!(10, "gallery-companion"),
        "gallery-relic" => preview!(11, "gallery-relic"),
        "gallery-nocturne" => preview!(12, "gallery-nocturne"),
        "gallery-marquee" => preview!(13, "gallery-marquee"),
        "gallery-ritual" => preview!(14, "gallery-ritual"),
        "gallery-ember" => preview!(15, "gallery-ember"),
        "gallery-volt" => preview!(16, "gallery-volt"),
        "gallery-aloft" => preview!(17, "gallery-aloft"),
        "gallery-gallery" => preview!(18, "gallery-gallery"),
        "gallery-nightcap" => preview!(19, "gallery-nightcap"),
        "gallery-bloom" => preview!(20, "gallery-bloom"),
        "freeform-wander" => preview!(21, "freeform-wander"),
        "freeform-forage" => preview!(22, "freeform-forage"),
        "freeform-still" => preview!(23, "freeform-still"),
        "freeform-hearth" => preview!(24, "freeform-hearth"),
        "freeform-meteo" => preview!(25, "freeform-meteo"),
        "freeform-marginalia" => preview!(26, "freeform-marginalia"),
        "freeform-lingua" => preview!(27, "freeform-lingua"),
        "freeform-daybreak" => preview!(28, "freeform-daybreak"),
        "freeform-verdant" => preview!(29, "freeform-verdant"),
        "freeform-companion" => preview!(30, "freeform-companion"),
        "freeform-relic" => preview!(31, "freeform-relic"),
        "freeform-nocturne" => preview!(32, "freeform-nocturne"),
        "freeform-marquee" => preview!(33, "freeform-marquee"),
        "freeform-ritual" => preview!(34, "freeform-ritual"),
        "freeform-ember" => preview!(35, "freeform-ember"),
        "freeform-volt" => preview!(36, "freeform-volt"),
        "freeform-aloft" => preview!(37, "freeform-aloft"),
        "freeform-gallery" => preview!(38, "freeform-gallery"),
        "freeform-nightcap" => preview!(39, "freeform-nightcap"),
        "freeform-bloom" => preview!(40, "freeform-bloom"),
        "extreme-weather" => preview!(41, "extreme-weather"),
        "extreme-now-playing" => preview!(42, "extreme-now-playing"),
        "extreme-daily-app" => preview!(43, "extreme-daily-app"),
        "extreme-calendar" => preview!(44, "extreme-calendar"),
        "extreme-calm" => preview!(45, "extreme-calm"),
        "web-orbit" => preview!(46, "web-orbit"),
        "web-atelier" => preview!(47, "web-atelier"),
        "dashboard-pulse" => preview!(48, "dashboard-pulse"),
        "dashboard-sentinel" => preview!(49, "dashboard-sentinel"),
        "component-data-grid" => preview!(50, "component-data-grid"),
        "component-form-lab" => preview!(51, "component-form-lab"),
        "modify-polish-current" => preview!(52, "modify-polish-current"),
        "modify-complete-states" => preview!(53, "modify-complete-states"),
        "starter-travel-app" => preview!(54, "starter-travel-app"),
        "starter-dashboard" => preview!(55, "starter-dashboard"),
        "starter-coffee-shop" => preview!(56, "starter-coffee-shop"),
        "starter-barbershop" => preview!(57, "starter-barbershop"),
        "web-kilnform" => preview!(58, "web-kilnform"),
        "web-reefwright" => preview!(59, "web-reefwright"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use op_editor_core::prompt_center_catalog::prompt_catalogue;
    use serde_json::Value;

    use super::prompt_center_preview;

    #[test]
    fn every_accepted_catalogue_preview_has_one_unique_image() {
        let generated = prompt_catalogue();
        assert_eq!(generated.len(), 59);

        let mut image_ids = HashSet::new();
        for prompt in generated {
            let preview = prompt_center_preview(&prompt.id)
                .unwrap_or_else(|| panic!("missing preview for `{}`", prompt.id));
            assert!(
                image_ids.insert(preview.image_id),
                "duplicate image id {:#x}",
                preview.image_id
            );
            let bytes = preview.bytes.expect("native embeds every preview");
            assert_eq!(
                crate::image_runtime::encoded_image_dimensions(bytes),
                Some((640, 400)),
                "wrong preview dimensions for `{}`",
                prompt.id
            );
        }
        assert_eq!(image_ids.len(), 59);
    }

    #[test]
    fn route_prefix_matches_the_shared_constant() {
        // `concat!` cannot interpolate a const, so the macro spells the prefix
        // out. This is what stops the literal and the daemon's route from
        // drifting apart into a silent 404 on every card.
        let preview = prompt_center_preview("gallery-wander").expect("ships");
        assert!(preview
            .route
            .starts_with(op_editor_core::web_assets::WEB_ASSET_ROUTE_PREFIX));
        assert_eq!(
            preview.route,
            format!(
                "{}{}/gallery-wander.jpg",
                op_editor_core::web_assets::WEB_ASSET_ROUTE_PREFIX,
                super::PREVIEW_DIR
            )
        );
    }

    #[test]
    fn every_shipped_preview_has_a_distinct_route() {
        // A duplicated route would make two cards share one fetch and one
        // picture — the routes are the web host's identity for these assets
        // exactly as the image ids are the renderer's.
        let mut routes = HashSet::new();
        for prompt in prompt_catalogue() {
            let preview = prompt_center_preview(&prompt.id).expect("ships");
            assert!(
                routes.insert(preview.route),
                "duplicate route {}",
                preview.route
            );
        }
        assert_eq!(routes.len(), 59);
    }

    #[test]
    fn custom_and_unknown_ids_have_no_generated_preview() {
        assert!(prompt_center_preview("custom-1").is_none());
        assert!(prompt_center_preview("unknown-prompt").is_none());
    }

    #[test]
    fn every_generated_preview_has_accepted_model_provenance() {
        let provenance: Value = serde_json::from_str(include_str!(concat!(
            "../../assets/prompt_center_previews/",
            "preview_provenance.json"
        )))
        .expect("preview provenance must be valid JSON");
        let entries = provenance["entries"]
            .as_object()
            .expect("preview provenance must have an entries object");
        let generated = prompt_catalogue();
        assert_eq!(entries.len(), generated.len());

        for prompt in generated {
            let entry = entries
                .get(&prompt.id)
                .unwrap_or_else(|| panic!("missing provenance for `{}`", prompt.id));
            assert_eq!(entry["status"], "accepted", "{}", prompt.id);
            assert!(
                entry["provider"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{}",
                prompt.id
            );
            assert!(
                entry["model"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{}",
                prompt.id
            );
            assert_eq!(
                entry["previewSha256"].as_str().map(str::len),
                Some(64),
                "{}",
                prompt.id
            );
        }
    }
}
