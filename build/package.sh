#!/bin/bash
set -euo pipefail

APP_NAME="ergo"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$SCRIPT_DIR"
TARGET_DIR="$PROJECT_DIR/target"
APP_DIR="$BUILD_DIR/$APP_NAME.app"
DMG_STAGING="$BUILD_DIR/$APP_NAME"
TARGETS=(x86_64-apple-darwin aarch64-apple-darwin)

cd "$PROJECT_DIR"

echo "[+] cleaning previous build artifacts"
rm -rf "$APP_DIR" "$DMG_STAGING" "$BUILD_DIR/$APP_NAME.dmg"
for target in "${TARGETS[@]}"; do
    arch="${target%%-*}"
    rm -f "$BUILD_DIR/$APP_NAME-$arch"
done

echo "[+] creating .app bundle"
mkdir -p "$APP_DIR/Contents/MacOS"
cp "$BUILD_DIR/Info.plist" "$APP_DIR/Contents/"

for target in "${TARGETS[@]}"; do
    arch="${target%%-*}"
    echo "[+] building $arch"
    cargo build --target="$target" --release
    cp "$TARGET_DIR/$target/release/$APP_NAME" "$BUILD_DIR/$APP_NAME-$arch"
done

echo "[+] creating universal binary"
lipo -create -output "$APP_DIR/Contents/MacOS/$APP_NAME" \
    "$BUILD_DIR/$APP_NAME-x86_64" \
    "$BUILD_DIR/$APP_NAME-aarch64"

echo "[+] creating dmg"
mkdir -p "$DMG_STAGING"
ln -s /Applications "$DMG_STAGING/Applications"
cp -r "$APP_DIR" "$DMG_STAGING/"
hdiutil create "$BUILD_DIR/$APP_NAME.dmg" -srcfolder "$DMG_STAGING" -ov

echo "[+] cleaning up"
rm -rf "$DMG_STAGING"
for target in "${TARGETS[@]}"; do
    arch="${target%%-*}"
    rm -f "$BUILD_DIR/$APP_NAME-$arch"
done

echo "[+] done: $BUILD_DIR/$APP_NAME.dmg"