#!/usr/bin/env bash
# Assemble a drop-in `Client/` folder for Windows from the cross-compiled exe:
#   client-rust/dist/Client/SR-ClientRadio.exe + AudioEffects/ + RadioModels/ + awacs-radios.json
# Usage (from anywhere):  client-rust/tools/package.sh [--build]
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"          # repository root
CR="$ROOT/client-rust"
EXE="$CR/target/x86_64-pc-windows-msvc/release/SR-ClientRadio.exe"
OUT="$CR/dist/Client"

if [[ "${1:-}" == "--build" ]]; then
  (cd "$CR" && cargo xwin build --release --target x86_64-pc-windows-msvc -p srs-app)
fi

[[ -f "$EXE" ]] || { echo "missing $EXE (run with --build)"; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT/RadioModels"
cp "$EXE" "$OUT/"
cp -R "$ROOT/DCS-SR-Client/AudioEffects" "$OUT/AudioEffects"
cp "$ROOT/DCS-SR-Client/RadioModels/arc210.json" "$OUT/RadioModels/"
cp "$ROOT/DCS-SR-Client/awacs-radios.json" "$OUT/"
cat > "$OUT/README-RUST.txt" <<'EOF'
SRS client (Rust build) - drop-in test package
==============================================
1. Copy this folder's content over the Client\ folder of an existing SRS install
   (keep a backup of the original SR-ClientRadio.exe), or run it from anywhere.
2. Run SR-ClientRadio.exe. No .NET runtime, no DLL needed. A console shows logs
   in debug builds; set RUST_LOG=debug for more.
3. Same arguments as the C# client: -host=ip:port  -cfg=dir  -allowMultiple
4. Settings are read from global.cfg / <profile>.cfg in the working directory.
Not ported (by design): audio recording, LotATC, VAICOM, auto-update, tray icon,
RefocusDCS, RequireAdmin, mic passthrough / audio preview.
Logs: clientlog.txt next to the exe.
EOF
VERSION="$(grep -m1 '^version' "$CR/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
(cd "$CR/dist" && rm -f "SRS-Client-Rust-$VERSION.zip" && zip -qr "SRS-Client-Rust-$VERSION.zip" Client)
echo "packaged: $CR/dist/SRS-Client-Rust-$VERSION.zip"
ls -la "$OUT"
