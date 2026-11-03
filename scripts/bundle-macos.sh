#!/usr/bin/env bash
# macOS için "DGMOS Drive.app" paketini üretir: release derleme, .icns ikonu
# ve Info.plist. Çıktı: target/release/bundle/DGMOS Drive.app
set -euo pipefail

APP_NAME="DGMOS Drive"
BUNDLE_ID="com.dgmos.drive"
BIN="dgmos-drive"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
OUT="$ROOT/target/release/bundle"
APP="$OUT/$APP_NAME.app"

# BIN_PATH verilirse (CI'daki universal binary) derleme atlanır.
if [[ -z "${BIN_PATH:-}" ]]; then
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
  BIN_PATH="$ROOT/target/release/$BIN"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# Çalıştırılabilir dosya uygulama adını taşır; Aktivite Monitörü ve Dock bu adı gösterir.
cp "$BIN_PATH" "$APP/Contents/MacOS/$APP_NAME"

# .icns: assets/icon-1024.png'den tüm standart boyutlar
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size "$ROOT/assets/icon-1024.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$ROOT/assets/icon-1024.png" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>$APP_NAME</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleDevelopmentRegion</key><string>tr</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSHumanReadableCopyright</key><string>© $(date +%Y) DG Regtech Co.</string>
</dict>
</plist>
PLIST

# İmzasız paketler Gatekeeper'da "hasarlı" görünebilir; yerel ad-hoc imza ekle.
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true

echo "$APP"
