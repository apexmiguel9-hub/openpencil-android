# OpenPencil Android ARM64

Android ARM64 build of [OpenPencil](https://github.com/ZSeven-W/openpencil) v0.8.4.

This repository contains the minimal structure needed to build an Android ARM64 APK via GitHub Actions.

## Project Structure

```
openpencil-android/
├── .github/workflows/android.yml    # GitHub Actions workflow for ARM64 build
├── packaging/android/               # Android app module (Gradle)
│   ├── app/                         # Main application (Kotlin + JNI)
│   ├── build.gradle.kts             # Root Gradle config
│   ├── settings.gradle.kts          # Gradle settings
│   ├── gradle.properties            # Gradle properties
│   ├── gradle/wrapper/              # Gradle wrapper
│   └── gradlew / gradlew.bat        # Gradle wrapper scripts
├── crates/                          # Rust crates (core engine)
│   ├── op-engine-jni/               # Android JNI bindings
│   ├── op-engine-ffi/               # C ABI for embedding
│   ├── op-editor-core/              # Editor state & document model
│   ├── op-editor-ui/                # Platform-free widgets
│   ├── op-host-native/              # Native host (Skia + jian)
│   ├── op-pen-loader/               # .op document loader
│   ├── op-editor-host-core/         # Host state machines
│   ├── op-collab-host/              # Collaboration runtime
│   ├── op-collab/                   # Collaboration protocol
│   ├── op-collab-transport/         # Transport layer
│   ├── op-ai/                       # AI integration
│   ├── op-ai-skills/                # AI skill corpus
│   ├── op-orchestrator/             # AI orchestration
│   ├── op-image-enrich/             # Image search/generation
│   ├── op-render-export/            # Export (PNG/SVG/PDF)
│   ├── op-builtin-model-discovery/  # Model discovery
│   ├── op-chat-agent/               # Chat agent
│   ├── op-auth-bridge/              # Auth bridge
│   ├── op-config-store/             # Config persistence
│   ├── op-util/                     # Utilities
│   ├── op-i18n/                     # Internationalization
│   └── ...                          # Other crates
├── vendor/jian/                     # Rendering submodule (Skia + layout)
├── scripts/build-android.sh         # Local build script
├── scripts/android-version.sh       # Version extraction
├── scripts/workspace-version.sh     # Workspace version
├── tools/install-pinned-android-sdk.sh
├── tools/check-android-release-workflow.sh
├── Cargo.toml                       # Workspace root
├── Cargo.lock
├── rust-toolchain.toml              # Rust 1.94
└── rustfmt.toml
```

## Requirements

### For Local Build
- **Java 21+** (JDK)
- **Rust 1.94** (via rustup)
- **Android SDK** (API 36, Build Tools 36.0.0)
- **Android NDK** r28c (28.2.13676358)
- **cargo-ndk** (`cargo install cargo-ndk`)

Environment variables:
```bash
export ANDROID_HOME=/path/to/android-sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/28.2.13676358
```

### For GitHub Actions
All dependencies are installed automatically in the workflow.

## Building

### Local Build
```bash
# Make script executable
chmod +x scripts/build-android.sh

# Run build
./scripts/build-android.sh
```

The script will:
1. Verify prerequisites (Java, Rust, Android SDK/NDK)
2. Configure environment for cross-compilation
3. Build `libop_engine_jni.so` for `arm64-v8a` via `cargo ndk`
4. Build debug APK via Gradle
5. Verify APK contains the native library

Output: `packaging/android/app/build/outputs/apk/debug/app-debug.apk`

### GitHub Actions
Push to `main`/`master` branch or run workflow manually:

```bash
git push origin main
```

The workflow will:
1. Checkout with submodules
2. Setup Java 21, Rust 1.94, cargo-ndk
3. Install Android SDK/NDK (pinned versions)
4. Build `libop_engine_jni.so` for `aarch64-linux-android`
5. Build debug APK
6. Verify APK structure and native library
7. Upload `openpencil-android-arm64-debug.apk` as artifact

## Toolchain Versions (Pinned)

| Component | Version |
|-----------|---------|
| Rust | 1.94 |
| Java | 21 (Temurin) |
| Gradle | 8.14.3 |
| Android Gradle Plugin | 8.13.2 |
| Kotlin | 2.0.21 |
| Android SDK Platform | 36 |
| Android Build Tools | 36.0.0 |
| Android NDK | 28.2.13676358 |

## Architecture

```
Android App (Kotlin)
    │
    ├── OpSurfaceView (SurfaceView + Choreographer)
    ├── OpSurfaceViewEditorTouch (gestures: pan, pinch, tap)
    ├── OpInputConnection (IME)
    └── OpNative (JNI bindings)
            │
            ▼
libop_engine_jni.so (Rust cdylib)
    │
    ├── op-engine-jni (JNI marshalling, EngineThread, Registry)
    └── op-engine-ffi (C ABI: op_create, op_frame, op_pointer, op_attach_surface)
            │
            ▼
op-engine-ffi internals
    │
    ├── Session (EditorState, LayoutScene, NativeBackend)
    ├── SurfaceSlot::Egl (jian_skia::surface::egl_android)
    │   ├── EGLDisplay / EGLContext / EGLSurface
    │   └── OpenGL ES 3.x
    ├── paint_frame → NativeFrameBackend → RenderBackend → Skia draw ops
    └── frame_gpu → egl.draw_frame → eglSwapBuffers
```

## Graphics Backend

**Android uses OpenGL ES 3.x via EGL** (not Vulkan):

```
SurfaceView → ANativeWindow → EGL → GLES 3.x → Skia (GPU backend)
```

- `jian_skia::surface::egl_android::EglSurface` wraps EGL surface
- Skia `DirectContext` created via `make_gl()` (GL backend)
- No Vulkan code path exists in current codebase

## APK Verification

The workflow validates:
- ✅ APK exists
- ✅ Contains `lib/arm64-v8a/libop_engine_jni.so`
- ✅ Package name: `tech.zseven.openpencil`
- ✅ Native library is valid ELF64 aarch64
- ✅ APK size reported

## License

MIT — same as upstream OpenPencil.

## Upstream

Based on [ZSeven-W/openpencil](https://github.com/ZSeven-W/openpencil) v0.8.4.