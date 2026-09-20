//! Applying the viewBox-aware root scale to element attributes,
//! path `d` data and point lists.

use super::*;

/// Multiply every numeric coord on the element by `scale` so a
/// 24-unit viewBox shape renders at the same final size as the TS
/// app's viewBox-aware path. For `<path>` we tokenise the `d`
/// string + scale every coord; for shape elements we scale
/// `width` / `height` / `x` / `y` / `r` / `cx` / `cy` / `rx` /
/// `ry` / `x1` / `y1` / `x2` / `y2` in place.
pub(super) fn apply_scale_to_attrs(el: &mut SvgElement, scale: f64) {
    if (scale - 1.0).abs() < 1e-6 {
        return;
    }
    if el.tag == "path" {
        if let Some(pos) = el.attrs.iter().position(|(k, _)| k == "d") {
            let scaled = scale_svg_path(&el.attrs[pos].1, scale);
            el.attrs[pos].1 = scaled;
        }
        return;
    }
    if el.tag == "polyline" || el.tag == "polygon" {
        if let Some(pos) = el.attrs.iter().position(|(k, _)| k == "points") {
            let scaled = scale_svg_points(&el.attrs[pos].1, scale);
            el.attrs[pos].1 = scaled;
        }
        return;
    }
    let scalable: &[&str] = &[
        "x", "y", "width", "height", "r", "rx", "ry", "cx", "cy", "x1", "y1", "x2", "y2",
    ];
    for (k, v) in &mut el.attrs {
        if !scalable.iter().any(|s| *s == k) {
            continue;
        }
        if let Ok(n) = v.trim().parse::<f64>() {
            *v = format!("{}", n * scale);
        }
    }
}

/// Token-aware scaler — preserves arc `A` flags (rotation +
/// large-arc + sweep are unitless) and scales the rest. Direct port
/// of `scaleSvgPath` from the TS impl.
fn scale_svg_path(d: &str, scale: f64) -> String {
    let mut out = String::with_capacity(d.len());
    let mut cmd: char = ' ';
    let mut param_idx = 0usize;
    let bytes = d.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() {
            cmd = c;
            param_idx = 0;
            out.push(c);
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() || c == ',' {
            out.push(c);
            i += 1;
            continue;
        }
        // Read a number (with optional sign / exponent).
        let start = i;
        if (c == '-' || c == '+') && i + 1 < bytes.len() {
            i += 1;
        }
        while i < bytes.len() {
            let cc = bytes[i] as char;
            if cc.is_ascii_digit() || cc == '.' {
                i += 1;
            } else if (cc == 'e' || cc == 'E')
                && i + 1 < bytes.len()
                && matches!(bytes[i + 1] as char, '-' | '+' | '0'..='9')
            {
                i += 1;
                if matches!(bytes[i] as char, '-' | '+') {
                    i += 1;
                }
            } else {
                break;
            }
        }
        if start == i {
            i += 1;
            continue;
        }
        let tok = &d[start..i];
        let Ok(n) = tok.parse::<f64>() else {
            out.push_str(tok);
            continue;
        };
        let upper = cmd.to_ascii_uppercase();
        let scaled = if upper == 'A' {
            // 7 params: rx ry rotation large-arc sweep x y
            let pos = param_idx % 7;
            let should_scale = pos == 0 || pos == 1 || pos == 5 || pos == 6;
            if should_scale {
                n * scale
            } else {
                n
            }
        } else {
            n * scale
        };
        out.push_str(&format!("{}", scaled));
        param_idx += 1;
    }
    out
}

/// Scale a `points="x1,y1 x2,y2 …"` list for `<polyline>` /
/// `<polygon>`. Returns the same separator style (`x,y x,y …`).
fn scale_svg_points(s: &str, scale: f64) -> String {
    parse_point_list(s)
        .into_iter()
        .map(|(x, y)| format!("{},{}", x * scale, y * scale))
        .collect::<Vec<_>>()
        .join(" ")
}
