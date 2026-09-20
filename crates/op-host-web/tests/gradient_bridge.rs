//! CanvasKit rounded-rectangle gradient bridge coverage.

fn bridge_source() -> String {
    std::fs::read_to_string(format!(
        "{}/src/op_ck_bridge.js",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("CanvasKit bridge source is readable")
}

#[test]
fn mesh_gradient_lattice_runs_in_javascript() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut source = bridge_source();
    source.push_str(
        r#"
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const colors = new Float32Array([
  1, 0, 0, 1,
  0, 1, 0, 0.75,
  0, 0, 1, 0.5,
  2, -1, 0.25, 1,
]);
const mesh = opCkBuildMeshGradientData(10, 20, 100, 50, 2, 2, colors);
assert(mesh !== null, 'a valid 2x2 lattice must be accepted');
assert(JSON.stringify(Array.from(mesh.positions)) === JSON.stringify([10, 20, 110, 20, 10, 70, 110, 70]),
  'vertices must cover the authored rectangle in row-major order');
assert(JSON.stringify(Array.from(mesh.indices)) === JSON.stringify([0, 1, 2, 1, 3, 2]),
  'one grid cell must triangulate into two consistently wound triangles');
assert(JSON.stringify(Array.from(mesh.rgba.slice(12))) === JSON.stringify([1, 0, 0.25, 1]),
  'vertex colour channels must be clamped before entering CanvasKit');
assert(opCkBuildMeshGradientData(0, 0, 10, 10, 2, 2, new Float32Array(12)) === null,
  'a malformed colour lattice must use the visible fallback path');
assert(opCkBuildMeshGradientData(0, 0, 10, 10, 256, 256, { length: 0 }) === null,
  'a grid that cannot be indexed by u16 must be rejected without allocating');
"#,
    );

    let mut child = match Command::new("node")
        .args(["--input-type=module"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("failed to start node for CanvasKit gradient test: {error}"),
    };
    child
        .stdin
        .take()
        .expect("node stdin is available")
        .write_all(source.as_bytes())
        .expect("CanvasKit gradient test source is writable");
    let output = child
        .wait_with_output()
        .expect("CanvasKit gradient JavaScript test completes");
    assert!(
        output.status.success(),
        "CanvasKit gradient JavaScript assertions failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rounded_gradient_methods_use_real_canvaskit_shaders_and_vertices() {
    let bridge = bridge_source();
    let rust = canvaskit_source();

    for method in [
        "fillRoundRectLinearGradient(",
        "fillRoundRectLinearGradientPerCorner(",
        "fillRoundRectRadialGradient(",
        "fillRoundRectRadialGradientPerCorner(",
        "fillRoundRectMeshGradient(",
        "fillRoundRectMeshGradientPerCorner(",
    ] {
        assert!(
            bridge.contains(method),
            "missing JS bridge method `{method}`"
        );
        assert!(
            rust.contains(&format!("js_name = {}", method.trim_end_matches('('))),
            "missing wasm-bindgen declaration for `{method}`"
        );
    }

    for marker in [
        "CK.Shader.MakeLinearGradient(",
        "CK.Shader.MakeRadialGradient(",
        "CK.MakeVertices(",
        "mesh.rgba,\n        mesh.indices,\n        true,",
        "canvas.drawVertices(vertices, CK.BlendMode.Modulate, paint)",
        "canvas.clipRRect(rrect, CK.ClipOp.Intersect, true)",
    ] {
        assert!(
            bridge.contains(marker),
            "gradient bridge must contain `{marker}`"
        );
    }

    let linear = bridge
        .find("const makeLinearGradient =")
        .expect("linear gradient helper");
    let radial = bridge
        .find("const makeRadialGradient =")
        .expect("radial gradient helper");
    let mesh = bridge
        .find("const drawMeshGradientRRect =")
        .expect("mesh gradient helper");
    assert!(linear < radial && radial < mesh);

    let linear_helper = &bridge[linear..radial];
    let radial_helper = &bridge[radial..mesh];
    assert!(
        linear_helper.contains("CK.TileMode.Clamp,\n        null,\n        1,"),
        "linear transparency must interpolate in premultiplied colour space"
    );
    assert!(
        radial_helper.contains("CK.TileMode.Clamp,\n        null,\n        1,"),
        "radial transparency must interpolate in premultiplied colour space"
    );
}

/// The CanvasKit host source: the `canvaskit.rs` spine plus every sibling
/// module under `canvaskit/` (the file was split at the 800-line ceiling).
fn canvaskit_source() -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut parts = vec![std::fs::read_to_string(format!("{root}/canvaskit.rs"))
        .expect("canvaskit spine is readable")];
    let mut siblings: Vec<std::path::PathBuf> = std::fs::read_dir(format!("{root}/canvaskit"))
        .expect("canvaskit module directory is readable")
        .map(|entry| entry.expect("canvaskit module entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    siblings.sort();
    for path in siblings {
        parts.push(std::fs::read_to_string(&path).expect("canvaskit module is readable"));
    }
    parts.join("\n")
}
