#!/usr/bin/env python3
"""从透明 RGBA 源图生成 Tauri / 应用图标（保留 alpha）。"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "assets" / "xiaohan-pet-icon-source-rgba.png"
ICONS = ROOT / "src-tauri" / "icons"
PUBLIC = ROOT / "public" / "app-icon.png"


def main() -> None:
    if not SRC.exists():
        raise SystemExit(f"源图不存在: {SRC}")

    src = Image.open(SRC).convert("RGBA")
    ICONS.mkdir(parents=True, exist_ok=True)

    sizes = {
        "32x32.png": 32,
        "64x64.png": 64,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    for name, px in sizes.items():
        out = ICONS / name
        src.resize((px, px), Image.Resampling.LANCZOS).save(out, format="PNG")
        print(f"wrote {out}")

    ico_layers = [src.resize((s, s), Image.Resampling.LANCZOS) for s in (16, 24, 32, 48, 64, 128, 256)]
    ico_path = ICONS / "icon.ico"
    ico_layers[0].save(
        ico_path,
        format="ICO",
        sizes=[(img.width, img.height) for img in ico_layers],
        append_images=ico_layers[1:],
    )
    print(f"wrote {ico_path}")

    src.resize((256, 256), Image.Resampling.LANCZOS).save(PUBLIC, format="PNG")
    print(f"wrote {PUBLIC}")


if __name__ == "__main__":
    main()
