//! CSS `transform` / `transform-origin` parsing, composition and baking.
//!
//! The Jian node schema has no transform matrix: a node carries `x` / `y`,
//! a single `rotation` in degrees, and a concrete width / height. The
//! importer therefore composes the whole CSS transform list into one 2-D
//! affine matrix (taken around `transform-origin`), decomposes it, and bakes
//! the translation into `x` / `y`, the rotation into `rotation`, and the two
//! scale factors into the node's size. Skew, residual shear and 3-D
//! functions cannot be expressed and degrade through `warn_once`.

use crate::css::cascade::ComputedStyle;
use crate::import_warning::ImportWarning;
use crate::length::{parse_length, LengthCtx};
use crate::mapper::MapCtx;

/// A 2-D affine matrix in the CSS `matrix(a, b, c, d, e, f)` convention:
/// `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// `self · rhs` — the CSS list order, so `rhs` is applied to the point
    /// first and `self` second.
    pub fn multiply(self, rhs: Self) -> Self {
        Self {
            a: self.a * rhs.a + self.c * rhs.b,
            b: self.b * rhs.a + self.d * rhs.b,
            c: self.a * rhs.c + self.c * rhs.d,
            d: self.b * rhs.c + self.d * rhs.d,
            e: self.a * rhs.e + self.c * rhs.f + self.e,
            f: self.b * rhs.e + self.d * rhs.f + self.f,
        }
    }

    pub fn translate(tx: f64, ty: f64) -> Self {
        Self {
            e: tx,
            f: ty,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            d: sy,
            ..Self::IDENTITY
        }
    }

    pub fn rotate_degrees(degrees: f64) -> Self {
        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            ..Self::IDENTITY
        }
    }

    pub fn skew_degrees(x_degrees: f64, y_degrees: f64) -> Self {
        Self {
            c: x_degrees.to_radians().tan(),
            b: y_degrees.to_radians().tan(),
            ..Self::IDENTITY
        }
    }

    /// Map an element-local point through the matrix.
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .into_iter()
            .all(f64::is_finite)
    }
}

/// Result of decomposing the composed matrix into the three components the
/// Jian schema can carry, plus the residual shear it cannot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decomposed {
    pub translate: (f64, f64),
    pub rotation_degrees: f64,
    pub scale: (f64, f64),
    /// `tan` of the residual XY skew angle. Non-zero means the matrix is not
    /// representable as translate + rotate + axis-aligned scale.
    pub shear: f64,
}

/// Standard `unmatrix` QR decomposition (CSS Transforms Level 1, §"Decomposing
/// a 2D matrix"). Scale can come out negative for a mirrored matrix; callers
/// bake the magnitude and warn about the flip.
pub fn decompose(matrix: Affine) -> Decomposed {
    let Affine { a, b, c, d, e, f } = matrix;
    let mut row0 = (a, b);
    let mut row1 = (c, d);
    let mut scale_x = (row0.0 * row0.0 + row0.1 * row0.1).sqrt();
    if scale_x > 0.0 {
        row0 = (row0.0 / scale_x, row0.1 / scale_x);
    }
    let mut shear = row0.0 * row1.0 + row0.1 * row1.1;
    row1 = (row1.0 - row0.0 * shear, row1.1 - row0.1 * shear);
    let mut scale_y = (row1.0 * row1.0 + row1.1 * row1.1).sqrt();
    if scale_y > 0.0 {
        row1 = (row1.0 / scale_y, row1.1 / scale_y);
        shear /= scale_y;
    }
    // A negative determinant means one axis is mirrored. Either axis can carry
    // the flip; pick the one that leaves the smaller residual rotation. Always
    // flipping X turns `scaleY(-1)` into a −180° rotation, which imports the
    // node upside down instead of merely dropping the mirror.
    if row0.0 * row1.1 - row0.1 * row1.0 < 0.0 {
        let flip_x_rotation = (-row0.1).atan2(-row0.0);
        let flip_y_rotation = row0.1.atan2(row0.0);
        if flip_x_rotation.abs() <= flip_y_rotation.abs() {
            scale_x = -scale_x;
            row0 = (-row0.0, -row0.1);
        } else {
            // Rotation is read off row0 alone, so flipping the Y axis leaves
            // the angle untouched and only negates the vertical scale.
            scale_y = -scale_y;
        }
    }
    Decomposed {
        translate: (e, f),
        rotation_degrees: row0.1.atan2(row0.0).to_degrees(),
        scale: (scale_x, scale_y),
        shear,
    }
}

/// Everything the mapper needs to bake one element's transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformBake {
    pub translate: (f64, f64),
    pub rotation_degrees: f64,
    pub scale: (f64, f64),
    /// Where the composed matrix sends the element's centre, plus the box the
    /// centre was taken from. The mapper re-derives `translate` from these two
    /// when `bake_scale` could only apply the scale to some axes.
    centre: (f64, f64),
    own: (f64, f64),
}

impl TransformBake {
    /// Both thresholds compare against the 1e-6 rounding grid the bake snaps
    /// its outputs to, so "moves" and "resizes" agree on what counts as dust.
    pub fn moves_node(&self) -> bool {
        self.translate.0.abs() > 1e-6 || self.translate.1.abs() > 1e-6
    }

    pub fn resizes_node(&self) -> bool {
        (self.scale.0 - 1.0).abs() > 1e-6 || (self.scale.1 - 1.0).abs() > 1e-6
    }

    /// The centre-derived translation for the case where only `applied` axes
    /// actually received the scale. An axis the mapper declined to scale keeps
    /// its original extent, so its pull-back is half of the UNSCALED box.
    pub fn translate_for_applied_scale(&self, applied: (bool, bool)) -> (f64, f64) {
        if applied == (true, true) {
            return self.translate;
        }
        let factor = |applied: bool, scale: f64| if applied { scale } else { 1.0 };
        (
            finite_or_zero(self.centre.0 - self.own.0 * factor(applied.0, self.scale.0) / 2.0),
            finite_or_zero(self.centre.1 - self.own.1 * factor(applied.1, self.scale.1) / 2.0),
        )
    }
}

/// Parse, compose around `transform-origin` and decompose the element's
/// `transform`. `own_width` / `own_height` are the element's used border-box
/// size, which percentage translations and the origin resolve against;
/// `definite` says whether each of those two numbers is a real used size or
/// the containing block standing in for an auto axis.
pub fn bake_transform(
    style: &ComputedStyle,
    context: &mut MapCtx<'_>,
    own_width: f64,
    own_height: f64,
    definite: (bool, bool),
) -> Option<TransformBake> {
    let value = style.get("transform")?;
    if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("none") {
        return None;
    }
    let length_ctx = length_ctx(style, context);
    let Some(functions) = split_functions(value) else {
        context.warn_once(ImportWarning::TransformUnsupportedSyntax);
        return None;
    };
    let function_ctx = FunctionCtx {
        length_ctx: &length_ctx,
        own: (own_width, own_height),
        definite,
    };
    let mut matrix = Affine::IDENTITY;
    let mut unsupported = false;
    let mut dropped_percentage = false;
    for (name, arguments) in &functions {
        match function_matrix(
            name,
            arguments,
            &function_ctx,
            &mut unsupported,
            &mut dropped_percentage,
        ) {
            Some(next) => matrix = matrix.multiply(next),
            None => unsupported = true,
        }
    }
    if unsupported {
        context.warn_once(ImportWarning::TransformUnsupportedFunction);
    }
    if dropped_percentage {
        context.warn_once(ImportWarning::TransformPercentageTranslationDropped);
    }
    if !matrix.is_finite() {
        context.warn_once(ImportWarning::TransformNonFiniteMatrix);
        return None;
    }
    let (origin_x, origin_y) = transform_origin(style, &length_ctx, own_width, own_height, context);
    let matrix = Affine::translate(origin_x, origin_y)
        .multiply(matrix)
        .multiply(Affine::translate(-origin_x, -origin_y));
    let decomposed = decompose(matrix);
    if decomposed.shear.abs() > 1e-6 {
        context.warn_once(ImportWarning::TransformSkewDropped);
    }
    let scale = (
        finite_scale(decomposed.scale.0),
        finite_scale(decomposed.scale.1),
    );
    if is_degenerate(decomposed.scale.0) || is_degenerate(decomposed.scale.1) {
        context.warn_once(ImportWarning::TransformDegenerateScale);
    } else if decomposed.scale.0 < 0.0 || decomposed.scale.1 < 0.0 {
        context.warn_once(ImportWarning::TransformMirroringAbsolute);
    }
    // The schema rotates a node about its own centre and stores the size on
    // the node, so the bakeable translation is how far the element's CENTRE
    // moved, minus half of the scaled box. A pure `rotate()` therefore comes
    // out as a zero translation, exactly as CSS leaves layout untouched.
    let centre = matrix.apply(own_width / 2.0, own_height / 2.0);
    let translate = (
        finite_or_zero(centre.0 - own_width * scale.0 / 2.0),
        finite_or_zero(centre.1 - own_height * scale.1 / 2.0),
    );
    Some(TransformBake {
        translate,
        rotation_degrees: finite_or_zero(decomposed.rotation_degrees),
        scale,
        centre,
        own: (own_width, own_height),
    })
}

/// `scale(0)` (the hidden-popover idiom) and non-finite factors have no node
/// representation: the schema cannot store a zero-extent box.
fn is_degenerate(value: f64) -> bool {
    !value.is_finite() || value.abs() <= f64::EPSILON
}

/// Trigonometry leaves 1e-15 dust on values authors wrote exactly (a
/// `rotate(15deg)` decomposes to 14.999999999999998). Snap to a
/// design-tool-meaningful precision so imported numbers read as authored.
fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1e6).round() / 1e6
    } else {
        0.0
    }
}

fn finite_scale(value: f64) -> f64 {
    let value = value.abs();
    if value.is_finite() && value > 0.0 {
        (value * 1e6).round() / 1e6
    } else {
        1.0
    }
}

fn length_ctx(style: &ComputedStyle, context: &MapCtx<'_>) -> LengthCtx {
    LengthCtx {
        font_size: style.font_size,
        root_font_size: context.opts.base_font_size,
        viewport_w: context.opts.viewport_width,
        viewport_h: context.opts.viewport_height(),
    }
}

/// Split a transform list into `(lowercase name, raw argument list)` pairs.
/// Returns `None` when the value is not a well-formed function list.
pub fn split_functions(value: &str) -> Option<Vec<(String, Vec<String>)>> {
    let mut functions = Vec::new();
    let mut name = String::new();
    let mut chars = value.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch.is_whitespace() || ch == ',' {
            if !name.is_empty() {
                return None;
            }
            continue;
        }
        if ch != '(' {
            name.push(ch.to_ascii_lowercase());
            continue;
        }
        if name.is_empty() || functions.len() >= MAX_FUNCTIONS {
            return None;
        }
        let mut depth = 1usize;
        let start = index + 1;
        let mut end = None;
        for (inner_index, inner) in chars.by_ref() {
            match inner {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(inner_index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end?;
        functions.push((
            std::mem::take(&mut name),
            split_arguments(&value[start..end]),
        ));
    }
    (name.is_empty() && !functions.is_empty()).then_some(functions)
}

const MAX_FUNCTIONS: usize = 64;

/// Split on top-level commas so `calc(1px + 2px)` arguments stay intact.
fn split_arguments(inner: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in inner.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => arguments.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() || !arguments.is_empty() {
        arguments.push(current);
    }
    arguments
        .into_iter()
        .map(|argument| argument.trim().to_string())
        .collect()
}

/// The element-box facts every transform function resolves against.
struct FunctionCtx<'a> {
    length_ctx: &'a LengthCtx,
    own: (f64, f64),
    /// Whether `own.0` / `own.1` is a real used size rather than the
    /// containing block substituted for an auto axis.
    definite: (bool, bool),
}

fn function_matrix(
    name: &str,
    arguments: &[String],
    ctx: &FunctionCtx<'_>,
    unsupported: &mut bool,
    dropped_percentage: &mut bool,
) -> Option<Affine> {
    let (own_width, own_height) = ctx.own;
    // A percentage translation is a fraction of the element's OWN box. On an
    // auto-sized axis that number is fabricated from the containing block, so
    // resolving it would move the node by an arbitrary distance (the classic
    // `left:50%; translate(-50%,-50%)` modal lands at 0). Drop the component
    // instead and let the caller report the degradation.
    let length = |index: usize, axis_is_horizontal: bool, dropped: &mut bool| -> Option<f64> {
        let raw = arguments.get(index)?;
        let parsed = parse_length(raw, ctx.length_ctx)?;
        let (reference, definite) = if axis_is_horizontal {
            (own_width, ctx.definite.0)
        } else {
            (own_height, ctx.definite.1)
        };
        if !definite && parsed.depends_on_reference() {
            *dropped = true;
            return Some(0.0);
        }
        Some(parsed.resolve(reference))
    };
    let number = |index: usize| -> Option<f64> {
        arguments
            .get(index)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
    };
    match name {
        "none" => Some(Affine::IDENTITY),
        "translate" => Some(Affine::translate(
            length(0, true, dropped_percentage)?,
            length(1, false, dropped_percentage).unwrap_or(0.0),
        )),
        "translatex" => Some(Affine::translate(length(0, true, dropped_percentage)?, 0.0)),
        "translatey" => Some(Affine::translate(
            0.0,
            length(0, false, dropped_percentage)?,
        )),
        "translate3d" => {
            if arguments.len() != 3 {
                return None;
            }
            Some(Affine::translate(
                length(0, true, dropped_percentage)?,
                length(1, false, dropped_percentage).unwrap_or(0.0),
            ))
        }
        "scale" => {
            let sx = number(0)?;
            Some(Affine::scale(sx, number(1).unwrap_or(sx)))
        }
        "scalex" => Some(Affine::scale(number(0)?, 1.0)),
        "scaley" => Some(Affine::scale(1.0, number(0)?)),
        "scale3d" => Some(Affine::scale(number(0)?, number(1)?)),
        "rotate" | "rotatez" => Some(Affine::rotate_degrees(parse_angle(arguments.first()?)?)),
        "rotate3d" => {
            // Only a pure Z-axis rotation survives a 2-D bake.
            let (x, y, z) = (number(0)?, number(1)?, number(2)?);
            let angle = parse_angle(arguments.get(3)?)?;
            if x.abs() > f64::EPSILON || y.abs() > f64::EPSILON || z.abs() <= f64::EPSILON {
                *unsupported = true;
                return Some(Affine::IDENTITY);
            }
            Some(Affine::rotate_degrees(angle * z.signum()))
        }
        "skew" => Some(Affine::skew_degrees(
            parse_angle(arguments.first()?)?,
            arguments
                .get(1)
                .and_then(|raw| parse_angle(raw))
                .unwrap_or(0.0),
        )),
        "skewx" => Some(Affine::skew_degrees(parse_angle(arguments.first()?)?, 0.0)),
        "skewy" => Some(Affine::skew_degrees(0.0, parse_angle(arguments.first()?)?)),
        "matrix" => {
            if arguments.len() != 6 {
                return None;
            }
            Some(Affine {
                a: number(0)?,
                b: number(1)?,
                c: number(2)?,
                d: number(3)?,
                e: number(4)?,
                f: number(5)?,
            })
        }
        _ => {
            // rotateX / rotateY / perspective / matrix3d and friends have no
            // 2-D reading; skip them but report the degradation.
            *unsupported = true;
            Some(Affine::IDENTITY)
        }
    }
}

pub fn parse_angle(value: &str) -> Option<f64> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    for (suffix, factor) in [
        ("deg", 1.0),
        ("grad", 0.9),
        ("turn", 360.0),
        ("rad", 180.0 / std::f64::consts::PI),
    ] {
        if let Some(number) = lower.strip_suffix(suffix) {
            return number
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| value * factor);
        }
    }
    // A bare `0` is the only unitless angle CSS accepts.
    lower
        .parse::<f64>()
        .ok()
        .filter(|value| *value == 0.0)
        .map(|_| 0.0)
}

/// Resolve `transform-origin` to element-local pixels. Defaults to the box
/// centre, matching the CSS initial value `50% 50%`.
pub fn transform_origin(
    style: &ComputedStyle,
    length_ctx: &LengthCtx,
    own_width: f64,
    own_height: f64,
    context: &mut MapCtx<'_>,
) -> (f64, f64) {
    let Some(raw) = style.get("transform-origin") else {
        return (own_width / 2.0, own_height / 2.0);
    };
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 3 {
        return (own_width / 2.0, own_height / 2.0);
    }
    if tokens.len() == 3 {
        context.warn_once(ImportWarning::TransformOriginZIgnored);
    }
    let mut horizontal = None;
    let mut vertical = None;
    let mut free = Vec::new();
    for token in tokens.iter().take(2) {
        match token.to_ascii_lowercase().as_str() {
            "left" => horizontal = Some(0.0),
            "right" => horizontal = Some(own_width),
            "top" => vertical = Some(0.0),
            "bottom" => vertical = Some(own_height),
            // `center`, lengths and percentages are axis-agnostic: they fill
            // the horizontal slot first, then the vertical one.
            _ => free.push(*token),
        }
    }
    // Slot assignment is positional, not value-driven: an unparseable first
    // token still consumes the horizontal slot so the second token cannot
    // silently slide onto the wrong axis.
    let mut horizontal_slot_taken = horizontal.is_some();
    for token in free {
        let axis_is_horizontal = !horizontal_slot_taken;
        let reference = if axis_is_horizontal {
            own_width
        } else {
            own_height
        };
        let resolved = if token.eq_ignore_ascii_case("center") {
            Some(reference / 2.0)
        } else {
            parse_length(token, length_ctx).map(|length| length.resolve(reference))
        };
        if axis_is_horizontal {
            horizontal_slot_taken = true;
            horizontal = resolved;
        } else {
            vertical = resolved;
        }
    }
    (
        horizontal.unwrap_or(own_width / 2.0),
        vertical.unwrap_or(own_height / 2.0),
    )
}

#[cfg(test)]
#[path = "transform_tests.rs"]
mod tests;
