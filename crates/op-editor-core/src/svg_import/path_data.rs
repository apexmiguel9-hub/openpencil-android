//! The `<path d="...">` reader: tokenizer plus the `M L H V C S Q T Z`
//! command subset, emitting canonical `PenPathAnchor`s.

use super::*;

/// Parse an SVG path `d` string into a list of subpaths. Each `M`
/// (after the first move) starts a new subpath — SVG's pen-up
/// semantics — so the renderer doesn't draw a stray straight line
/// between disconnected outlines. Returns `Vec<(anchors, closed)>`;
/// supports `M L H V C S Q T Z` (absolute + relative); `A` degrades
/// to a straight segment to its endpoint.
pub(super) fn parse_path_d(d: &str, offset: (f64, f64)) -> Vec<(Vec<PenPathAnchor>, bool)> {
    let tokens = tokenize_path(d);
    let (ox, oy) = offset;
    let mut subpaths: Vec<(Vec<PenPathAnchor>, bool)> = Vec::new();
    let mut anchors: Vec<PenPathAnchor> = Vec::new();
    let mut closed = false;
    // Current pen position, sub-path start, and the last control point
    // (for the smooth `S` / `T` reflection).
    let (mut cx, mut cy) = (0.0f64, 0.0f64);
    let (mut start_x, mut start_y) = (0.0f64, 0.0f64);
    let mut last_cubic_ctrl: Option<(f64, f64)> = None;
    let mut last_quad_ctrl: Option<(f64, f64)> = None;

    let push_anchor = |anchors: &mut Vec<PenPathAnchor>, x: f64, y: f64| {
        anchors.push(PenPathAnchor {
            x: x + ox,
            y: y + oy,
            handle_in: None,
            handle_out: None,
            point_type: None,
        });
    };

    let mut ti = 0usize;
    let mut cmd = b' ';
    while ti < tokens.len() {
        // A token is either a command letter or (when the previous
        // command repeats) a fresh number run.
        if let PathToken::Cmd(c) = tokens[ti] {
            cmd = c;
            ti += 1;
        }
        let rel = cmd.is_ascii_lowercase();
        let up = cmd.to_ascii_uppercase();
        // Collect the numbers this command consumes.
        let need = match up {
            b'M' | b'L' | b'T' => 2,
            b'H' | b'V' => 1,
            b'C' => 6,
            b'S' | b'Q' => 4,
            b'A' => 7,
            b'Z' => 0,
            _ => {
                ti += 1;
                continue;
            }
        };
        if up == b'Z' {
            closed = true;
            cx = start_x;
            cy = start_y;
            last_cubic_ctrl = None;
            last_quad_ctrl = None;
            continue;
        }
        let mut args = [0.0f64; 7];
        let mut got = 0;
        while got < need && ti < tokens.len() {
            if let PathToken::Num(n) = tokens[ti] {
                args[got] = n;
                got += 1;
                ti += 1;
            } else {
                break;
            }
        }
        if got < need {
            break; // truncated command — stop
        }
        match up {
            b'M' => {
                // Pen-up: a fresh `M` starts a new subpath. Flush the
                // current one (when it has ≥ 2 anchors) before starting
                // — otherwise multiple `M` commands in a single `d`
                // string get fused into one polyline with a stray
                // straight line between subpaths.
                if anchors.len() >= 2 {
                    subpaths.push((std::mem::take(&mut anchors), closed));
                } else {
                    anchors.clear();
                }
                closed = false;
                let (x, y) = abs_pt(rel, cx, cy, args[0], args[1]);
                cx = x;
                cy = y;
                start_x = x;
                start_y = y;
                push_anchor(&mut anchors, x, y);
                cmd = if rel { b'l' } else { b'L' }; // implicit lineto
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            b'L' => {
                let (x, y) = abs_pt(rel, cx, cy, args[0], args[1]);
                cx = x;
                cy = y;
                push_anchor(&mut anchors, x, y);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            b'H' => {
                let x = if rel { cx + args[0] } else { args[0] };
                cx = x;
                push_anchor(&mut anchors, x, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            b'V' => {
                let y = if rel { cy + args[0] } else { args[0] };
                cy = y;
                push_anchor(&mut anchors, cx, y);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            b'C' => {
                let (c1x, c1y) = abs_pt(rel, cx, cy, args[0], args[1]);
                let (c2x, c2y) = abs_pt(rel, cx, cy, args[2], args[3]);
                let (x, y) = abs_pt(rel, cx, cy, args[4], args[5]);
                emit_cubic(&mut anchors, c1x, c1y, c2x, c2y, x, y, ox, oy);
                cx = x;
                cy = y;
                last_cubic_ctrl = Some((c2x, c2y));
                last_quad_ctrl = None;
            }
            b'S' => {
                // Smooth cubic — first control reflects the previous.
                let (c1x, c1y) = match last_cubic_ctrl {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                let (c2x, c2y) = abs_pt(rel, cx, cy, args[0], args[1]);
                let (x, y) = abs_pt(rel, cx, cy, args[2], args[3]);
                emit_cubic(&mut anchors, c1x, c1y, c2x, c2y, x, y, ox, oy);
                cx = x;
                cy = y;
                last_cubic_ctrl = Some((c2x, c2y));
                last_quad_ctrl = None;
            }
            b'Q' => {
                let (qx, qy) = abs_pt(rel, cx, cy, args[0], args[1]);
                let (x, y) = abs_pt(rel, cx, cy, args[2], args[3]);
                let (c1x, c1y, c2x, c2y) = quad_to_cubic(cx, cy, qx, qy, x, y);
                emit_cubic(&mut anchors, c1x, c1y, c2x, c2y, x, y, ox, oy);
                cx = x;
                cy = y;
                last_quad_ctrl = Some((qx, qy));
                last_cubic_ctrl = None;
            }
            b'T' => {
                // Smooth quadratic — control reflects the previous.
                let (qx, qy) = match last_quad_ctrl {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                let (x, y) = abs_pt(rel, cx, cy, args[0], args[1]);
                let (c1x, c1y, c2x, c2y) = quad_to_cubic(cx, cy, qx, qy, x, y);
                emit_cubic(&mut anchors, c1x, c1y, c2x, c2y, x, y, ox, oy);
                cx = x;
                cy = y;
                last_quad_ctrl = Some((qx, qy));
                last_cubic_ctrl = None;
            }
            b'A' => {
                // Elliptical arc — v1 degrades to a straight segment to
                // the endpoint (args[5], args[6]).
                let (x, y) = abs_pt(rel, cx, cy, args[5], args[6]);
                cx = x;
                cy = y;
                push_anchor(&mut anchors, x, y);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            _ => {}
        }
    }
    // Flush the trailing subpath (no terminating `M` to flush it).
    if anchors.len() >= 2 {
        subpaths.push((anchors, closed));
    }
    subpaths
}

/// Resolve a possibly-relative point against the current pen pos.
fn abs_pt(rel: bool, cx: f64, cy: f64, x: f64, y: f64) -> (f64, f64) {
    if rel {
        (cx + x, cy + y)
    } else {
        (x, y)
    }
}

/// Convert a quadratic control point to the two cubic controls.
fn quad_to_cubic(x0: f64, y0: f64, qx: f64, qy: f64, x1: f64, y1: f64) -> (f64, f64, f64, f64) {
    (
        x0 + 2.0 / 3.0 * (qx - x0),
        y0 + 2.0 / 3.0 * (qy - y0),
        x1 + 2.0 / 3.0 * (qx - x1),
        y1 + 2.0 / 3.0 * (qy - y1),
    )
}

/// Append a cubic-curve segment, **preserving the bezier handles**
/// so the canvas painter's `flatten_path` redraws the smooth curve
/// at paint time. The previous anchor gets `handle_out = c1 − p0`;
/// the new anchor (endpoint) gets `handle_in = c2 − p3`. Both stored
/// as anchor-relative deltas — `path_anchor_bounds` + the layout-scene
/// builder agree on the relative convention.
///
/// Earlier this flattened cubics into 24 straight anchors at import
/// time, which dropped curve fidelity entirely. The canvas painter's
/// `flatten_path` already handles handles correctly, so flattening
/// here was both lossy and redundant.
// Each control point + endpoint + offset is its own scalar — bundling
// them into a struct would only obscure a flat geometric signature.
#[allow(clippy::too_many_arguments)]
fn emit_cubic(
    anchors: &mut Vec<PenPathAnchor>,
    c1x: f64,
    c1y: f64,
    c2x: f64,
    c2y: f64,
    x: f64,
    y: f64,
    ox: f64,
    oy: f64,
) {
    let (p0x, p0y) = match anchors.last() {
        Some(a) => (a.x, a.y),
        None => return,
    };
    let p3x = x + ox;
    let p3y = y + oy;
    if let Some(last) = anchors.last_mut() {
        last.handle_out = Some(PenPathHandle {
            x: c1x + ox - p0x,
            y: c1y + oy - p0y,
        });
    }
    anchors.push(PenPathAnchor {
        x: p3x,
        y: p3y,
        handle_in: Some(PenPathHandle {
            x: c2x + ox - p3x,
            y: c2y + oy - p3y,
        }),
        handle_out: None,
        point_type: None,
    });
}

/// A path-`d` lexer token.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PathToken {
    Cmd(u8),
    Num(f64),
}

/// Tokenize a path `d` string into command letters + numbers.
fn tokenize_path(d: &str) -> Vec<PathToken> {
    let bytes = d.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphabetic() {
            out.push(PathToken::Cmd(c));
            i += 1;
        } else if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut seen_dot = c == b'.';
            let mut seen_exp = false;
            while i < bytes.len() {
                let dch = bytes[i];
                if dch.is_ascii_digit() {
                    i += 1;
                } else if dch == b'.' && !seen_dot && !seen_exp {
                    seen_dot = true;
                    i += 1;
                } else if (dch == b'e' || dch == b'E') && !seen_exp {
                    seen_exp = true;
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(n) = d[start..i].parse::<f64>() {
                out.push(PathToken::Num(n));
            }
        } else {
            i += 1; // comma / whitespace separator
        }
    }
    out
}
