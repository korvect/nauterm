#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
allow_missing_python=false
if [[ "${1:-}" == --allow-missing-python ]]; then
  allow_missing_python=true
  shift
fi
MODE="${1:-full}"
case "$MODE" in
  full|--generated-only) ;;
  *)
    echo "Usage: $0 [--allow-missing-python] [--generated-only]" >&2
    exit 1
    ;;
esac

PYTHON=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'pass' >/dev/null 2>&1; then
    PYTHON="$candidate"
    break
  fi
done
if [[ -z "$PYTHON" ]]; then
  if "$allow_missing_python"; then
    if [[ "$MODE" == --generated-only ]]; then
      echo "Python is unavailable; skipping icon validation." >&2
    fi
    exit 0
  fi
  echo "Python is required to verify generated app icons." >&2
  exit 1
fi

"$PYTHON" - "$ROOT_DIR" "$MODE" <<'PY'
from hashlib import sha256
import json
from pathlib import Path
import struct
import sys

root = Path(sys.argv[1])
generated_only = sys.argv[2] == "--generated-only"
source_png = root / "assets/icons/app_icon.png"
source_composer = root / "assets/icons/Nauterm.icon"
generated_root = root / "assets/icons/generated"
generated_pngs = generated_root / "png"
source_hash_file = generated_root / "source.sha256"
generated_iconset = generated_root / "Nauterm.iconset"
generated_windows_icon = generated_root / "app_icon.ico"
mac_app_icons = root / "macos/Runner/Assets.xcassets/AppIcon.appiconset"
mac_composer = root / "macos/Runner/Resources/Nauterm.icon"
windows_icon = root / "windows/runner/resources/app_icon.ico"
linux_icon = root / "linux/runner/resources/nauterm.png"


def read_png(path: Path) -> tuple[bytes, int, int]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n" or len(data) < 24:
        raise SystemExit(f"Not a valid PNG: {path.relative_to(root)}")
    width, height = struct.unpack(">II", data[16:24])
    return data, width, height


def require_png_size(path: Path, size: int) -> bytes:
    data, width, height = read_png(path)
    if (width, height) != (size, size):
        raise SystemExit(
            f"{path.relative_to(root)} must be {size}x{size}, "
            f"got {width}x{height}"
        )
    return data


source_data = require_png_size(source_png, 1024)
expected_hash = sha256(source_data).hexdigest()
recorded_hash = source_hash_file.read_text(encoding="utf-8").split()[0]
if recorded_hash != expected_hash:
    raise SystemExit(
        "Generated app icons are stale. Run `make generate-icons` on macOS."
    )

pixel_sizes = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
generated: dict[int, bytes] = {
    size: require_png_size(
        generated_pngs / f"app_icon_{size}.png",
        size,
    )
    for size in pixel_sizes
}
if generated[1024] != source_data:
    raise SystemExit("Generated 1024px icon does not match app_icon.png")

iconset_sources = {
    "icon_16x16.png": 16,
    "icon_16x16@2x.png": 32,
    "icon_32x32.png": 32,
    "icon_32x32@2x.png": 64,
    "icon_128x128.png": 128,
    "icon_128x128@2x.png": 256,
    "icon_256x256.png": 256,
    "icon_256x256@2x.png": 512,
    "icon_512x512.png": 512,
    "icon_512x512@2x.png": 1024,
}
for filename, size in iconset_sources.items():
    if (generated_iconset / filename).read_bytes() != generated[size]:
        raise SystemExit(f"Generated iconset file is stale: {filename}")

composer = json.loads(
    (source_composer / "icon.json").read_text(encoding="utf-8")
)
composer_images = {
    layer["image-name"]
    for group in composer.get("groups", [])
    for layer in group.get("layers", [])
    if "image-name" in layer
}
for name in composer_images:
    composer_asset = source_composer / "Assets" / name
    if not composer_asset.is_file():
        raise SystemExit(
            "Icon Composer project references a missing asset: "
            f"{composer_asset.relative_to(root)}"
        )
    require_png_size(composer_asset, 1024)


def verify_ico(path: Path) -> None:
    ico = path.read_bytes()
    if len(ico) < 6:
        raise SystemExit(f"Windows app icon is truncated: {path.relative_to(root)}")
    reserved, icon_type, count = struct.unpack_from("<HHH", ico)
    expected_sizes = (16, 24, 32, 48, 64, 128, 256)
    if (reserved, icon_type, count) != (0, 1, len(expected_sizes)):
        raise SystemExit(
            f"Windows app icon has an unexpected header: {path.relative_to(root)}"
        )
    for index, expected_size in enumerate(expected_sizes):
        entry = struct.unpack_from("<BBBBHHII", ico, 6 + index * 16)
        width, height, _, _, planes, bit_count, length, offset = entry
        width = 256 if width == 0 else width
        height = 256 if height == 0 else height
        if (width, height, planes, bit_count) != (
            expected_size,
            expected_size,
            1,
            32,
        ):
            raise SystemExit(f"Windows ICO entry {index} has invalid metadata")
        payload = ico[offset : offset + length]
        if len(payload) != length or payload != generated[expected_size]:
            raise SystemExit(f"Windows ICO entry {index} has stale image data")


verify_ico(generated_windows_icon)

if generated_only:
    print("Generated app icon assets verified.")
    raise SystemExit(0)

mac_pngs: dict[int, bytes] = {}
for size in (16, 32, 64, 128, 256, 512, 1024):
    mac_pngs[size] = require_png_size(
        mac_app_icons / f"app_icon_{size}.png",
        size,
    )
    if mac_pngs[size] != generated[size]:
        raise SystemExit(f"Prepared macOS {size}px app icon is stale")

contents = json.loads(
    (mac_app_icons / "Contents.json").read_text(encoding="utf-8")
)
referenced_files = {
    image["filename"] for image in contents["images"] if "filename" in image
}
expected_files = {f"app_icon_{size}.png" for size in mac_pngs}
if referenced_files != expected_files:
    raise SystemExit("macOS AppIcon Contents.json does not reference every size")


def directory_hashes(path: Path) -> dict[str, str]:
    return {
        str(item.relative_to(path)): sha256(item.read_bytes()).hexdigest()
        for item in path.rglob("*")
        if item.is_file()
    }


if directory_hashes(source_composer) != directory_hashes(mac_composer):
    raise SystemExit(
        "Prepared macOS Icon Composer resource is stale. "
        "Run `make prepare-icons`."
    )

if linux_icon.read_bytes() != generated[1024]:
    raise SystemExit("Prepared Linux app icon is stale")
if windows_icon.read_bytes() != generated_windows_icon.read_bytes():
    raise SystemExit("Prepared Windows app icon is stale")
verify_ico(windows_icon)

print("Generated and prepared app icon resources verified.")
PY
