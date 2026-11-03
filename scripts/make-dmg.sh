#!/usr/bin/env bash
# Sürükle-bırak kurulumlu .dmg üretir: pencerede uygulama ve yanında
# Uygulamalar (Applications) kısayolu durur; kullanıcı uygulamayı oraya sürükler.
# Kullanım: scripts/make-dmg.sh "<DGMOS Drive.app>" <çıktı.dmg>
set -euo pipefail

APP="$1"
OUT="$2"
VOLNAME="DGMOS Drive"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

mkdir -p "$(dirname "$OUT")"
hdiutil create -volname "$VOLNAME" -srcfolder "$STAGE" -ov -format UDZO "$OUT" >/dev/null
echo "$OUT"
