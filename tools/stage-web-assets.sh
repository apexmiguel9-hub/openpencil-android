#!/usr/bin/env bash
# tools/stage-web-assets.sh — copy the runtime-fetched product assets into a
# web bundle's `assets/` directory.
#
# The wasm bundle deliberately omits these (see `op_editor_core::web_assets`):
# the browser fetches each on demand from `/pkg/assets/<dir>/<file>`, which the
# daemon serves straight out of the resolved bundle directory. The desktop
# binary still embeds them with `include_bytes!` / `include_str!`, so this
# script exists only for the web deployment.
#
# The layout under the destination MUST match the route literals in
# `prompt_center_previews.rs`, `scene_template_previews.rs`,
# `scene_template_catalog.rs`, `icon_catalog.rs` and `bundled_fonts_web.rs` —
# those are `concat!`ed / formatted at compile time, so a mismatch is a silent
# 404 per asset rather than a build error. The Rust side pins its half with
# route tests (the font manifest additionally asserts every listed file exists
# in the source directories staged below); this script is the other half.
#
# Usage: tools/stage-web-assets.sh <dest-assets-dir>
# Exit: 0 staged, 1 a source directory is missing.

set -euo pipefail

DEST="${1:?usage: stage-web-assets.sh <dest-assets-dir>}"
UI_ASSETS="crates/op-editor-ui/assets"
CORE_ASSETS="crates/op-editor-core/assets"
DESKTOP_FONTS="crates/op-host-desktop/assets/fonts"
NATIVE_ASSETS="crates/op-host-native/assets"

copy_dir() {
  local src="$1" name="$2"
  [ -d "${src}" ] || { printf 'FAIL: missing asset source %s\n' "${src}" >&2; exit 1; }
  mkdir -p "${DEST}/${name}"
  # `-R` not `-a`: no need to preserve ownership into a container image, and
  # BusyBox cp (the Docker build stage) has no `-a`.
  cp -R "${src}/." "${DEST}/${name}/"
  # Provenance manifests are a build-time record of which model produced each
  # preview; they are not fetched by anything and have no business in a
  # published bundle.
  rm -f "${DEST}/${name}/preview_provenance.json"
}

copy_file() {
  local src="$1" name="$2"
  [ -f "${src}" ] || { printf 'FAIL: missing asset source %s\n' "${src}" >&2; exit 1; }
  # `name` may carry a subdirectory (e.g. `fonts/Roboto-Regular.ttf`), so mkdir
  # the destination's parent rather than only `${DEST}`.
  mkdir -p "$(dirname "${DEST}/${name}")"
  cp "${src}" "${DEST}/${name}"
}

mkdir -p "${DEST}"
copy_dir "${UI_ASSETS}/prompt_center_previews" "prompt_center_previews"
copy_dir "${UI_ASSETS}/scene_template_previews" "scene_template_previews"
# Scene-template `.op` documents — fetched when a template is instantiated.
copy_dir "${CORE_ASSETS}/scene_templates" "scene_templates"
# Core (lucide + feather) icon catalog — fetched when the icon panel opens.
copy_file "${UI_ASSETS}/iconify-catalog-core.json" "iconify-catalog-core.json"
# The OFL design fonts the desktop binary embeds with `include_bytes!`. The
# browser has no bundled fonts of its own, so without these a document using
# e.g. Inter pops the missing-fonts modal and renders a fallback face; the web
# host fetches them all at mount (`op-host-web/src/bundled_fonts_web.rs`).
copy_dir "${DESKTOP_FONTS}" "fonts"
# Roboto lives with the native host (it is also that backend's last-resort
# face), not in the desktop font directory, so it is staged separately.
copy_file "${NATIVE_ASSETS}/Roboto-Regular.ttf" "fonts/Roboto-Regular.ttf"

staged="$(du -sk "${DEST}" | cut -f1)"
printf '  ✓ staged runtime assets into %s (%s KiB)\n' "${DEST}" "${staged}"
