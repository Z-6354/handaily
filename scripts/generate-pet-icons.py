#!/usr/bin/env python3
"""从 app-icon-square 源图生成 Tauri 全套图标（调用官方 tauri icon）。"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SRC = ROOT / "bundled" / "app-icon-square.png"
ICONS = ROOT / "src-tauri" / "icons"
PUBLIC = ROOT / "public" / "app-icon.png"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--source",
        type=Path,
        default=DEFAULT_SRC,
        help="方形透明底源图（默认 bundled/app-icon-square.png）",
    )
    args = parser.parse_args()
    src: Path = args.source.resolve()
    if not src.exists():
        raise SystemExit(f"源图不存在: {src}")

    print(f"[icons] source: {src}")
    subprocess.run(
        f'npx tauri icon "{src}" -o "{ICONS}"',
        cwd=ROOT,
        check=True,
        shell=True,
    )

    icon_png = ICONS / "icon.png"
    if icon_png.exists():
        shutil.copy2(icon_png, PUBLIC)
        print(f"wrote {PUBLIC}")

    # Windows 桌面仅需要核心文件，清理 tauri icon 顺带生成的移动端资源
    for sub in ("ios", "android"):
        path = ICONS / sub
        if path.exists():
            shutil.rmtree(path)
    for pattern in ("Square*.png", "StoreLogo.png", "icon.icns"):
        for path in ICONS.glob(pattern):
            path.unlink()

    print("[icons] done")


if __name__ == "__main__":
    main()
