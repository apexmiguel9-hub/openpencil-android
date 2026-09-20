#!/usr/bin/env bash
# Build OpenPencil Android ARM64 APK locally
# Mirrors the GitHub Actions workflow

set -euo pipefail

# Configuration
ANDROID_API_LEVEL=36
ANDROID_BUILD_TOOLS_VERSION=36.0.0
ANDROID_NDK_VERSION=28.2.13676358
RUST_TOOLCHAIN=1.94
CARGO_TARGET=aarch64-linux-android

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check for Java 21
    if ! command -v java &> /dev/null; then
        log_error "Java not found. Please install JDK 21."
        exit 1
    fi
    JAVA_VERSION=$(java -version 2>&1 | head -1 | cut -d'"' -f2 | cut -d'.' -f1)
    if [ "$JAVA_VERSION" -lt 21 ]; then
        log_warn "Java version $JAVA_VERSION found, recommended 21+"
    fi
    
    # Check for Rust
    if ! command -v rustc &> /dev/null; then
        log_error "Rust not found. Please install Rust $RUST_TOOLCHAIN"
        exit 1
    fi
    
    # Check for cargo-ndk
    if ! command -v cargo-ndk &> /dev/null; then
        log_warn "cargo-ndk not found. Installing..."
        cargo install cargo-ndk --locked
    fi
    
    # Check Android SDK
    if [ -z "${ANDROID_HOME:-}" ]; then
        log_error "ANDROID_HOME not set. Please set ANDROID_HOME to your Android SDK path."
        exit 1
    fi
    
    # Check NDK
    if [ ! -d "$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION" ]; then
        log_error "NDK $ANDROID_NDK_VERSION not found at $ANDROID_HOME/ndk/$ANDROID_NDK_VERSION"
        exit 1
    fi
    
    log_info "Prerequisites check passed"
}

# Setup environment
setup_env() {
    log_info "Setting up environment..."
    
    export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION"
    export ANDROID_SDK_ROOT="$ANDROID_HOME"
    export PATH="$PATH:$ANDROID_HOME/cmdline-tools/latest/bin"
    
    # Rust toolchain
    export RUSTUP_TOOLCHAIN="$RUST_TOOLCHAIN"
    source "$HOME/.cargo/env" 2>/dev/null || true
    
    # Cargo target configuration for Android
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=clang
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_CC="clang --target=aarch64-linux-android26 --sysroot=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_CXX="clang++ --target=aarch64-linux-android26 --sysroot=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR=llvm-ar
    export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--target=aarch64-linux-android26 --sysroot=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
    
    log_info "Environment configured"
}

# Build Rust JNI library
build_jni() {
    log_info "Building libop_engine_jni.so for arm64-v8a..."
    
    cargo ndk \
        -t arm64-v8a \
        -o packaging/android/app/src/debug/jniLibs \
        build -p op-engine-jni \
        --features gl,editor
    
    # Verify the library was built
    if [ ! -f "packaging/android/app/src/debug/jniLibs/arm64-v8a/libop_engine_jni.so" ]; then
        log_error "libop_engine_jni.so not found after build"
        exit 1
    fi
    
    log_info "JNI library built successfully"
    file packaging/android/app/src/debug/jniLibs/arm64-v8a/libop_engine_jni.so
}

# Build Android APK
build_apk() {
    log_info "Building Android APK with Gradle..."
    
    cd packaging/android
    chmod +x gradlew
    ./gradlew --no-daemon assembleDebug
    cd ../..
    
    log_info "APK built successfully"
}

# Verify APK
verify_apk() {
    log_info "Verifying APK..."
    
    APK_PATH=$(find packaging/android/app/build/outputs/apk/debug -name "*.apk" | head -1)
    
    if [ -z "$APK_PATH" ]; then
        log_error "No debug APK found"
        exit 1
    fi
    
    log_info "APK found at: $APK_PATH"
    
    # Verify native library
    if ! unzip -l "$APK_PATH" | grep -q "lib/arm64-v8a/libop_engine_jni.so"; then
        log_error "arm64-v8a native library not found in APK"
        exit 1
    fi
    
    # Verify package name
    if ! aapt dump badging "$APK_PATH" 2>/dev/null | grep -q "package: name='tech.zseven.openpencil'"; then
        log_error "Package name mismatch"
        exit 1
    fi
    
    # Get APK size
    APK_SIZE=$(stat -c%s "$APK_PATH")
    APK_SIZE_MB=$((APK_SIZE / 1024 / 1024))
    log_info "APK size: ${APK_SIZE_MB} MB"
    
    log_info "APK verification passed"
}

# Main
main() {
    log_info "Starting OpenPencil Android ARM64 build"
    log_info "====================================="
    
    check_prerequisites
    setup_env
    build_jni
    build_apk
    verify_apk
    
    log_info "====================================="
    log_info "Build completed successfully!"
    log_info "APK location: packaging/android/app/build/outputs/apk/debug/app-debug.apk"
}

main "$@"