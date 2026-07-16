#!/usr/bin/env python3
"""Merge UnityLive2DExtractor motions into data/model/unpacked/<slug>/.

Copies motions/*.motion3.json and merges FileReferences.Motions into existing
model3.json (keeps flat texture paths / HitAreas already present).
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def find_extractor(explicit: Path | None) -> Path:
    if explicit and explicit.is_file():
        return explicit
    root = repo_root() / "data/model/tools"
    candidates = list(root.rglob("UnityLive2DExtractor.exe"))
    if not candidates:
        raise FileNotFoundError(
            "UnityLive2DExtractor.exe not found under data/model/tools — "
            "download UnityLive2DExtractorMod (net8 portable) first"
        )
    return candidates[0]


def run_extractor(exe: Path, live2d_folder: Path) -> Path:
    """Extractor writes Live2DOutput next to CWD."""
    cwd = live2d_folder.parent
    # Avoid ReadKey crash when stdin redirected
    subprocess.run(
        [str(exe), str(live2d_folder)],
        cwd=str(cwd),
        input=b"\n",
        check=False,
    )
    out = cwd / "Live2DOutput"
    if not out.is_dir():
        # sometimes written beside tools cwd
        alt = repo_root() / "data/model/tools/Live2DOutput"
        if alt.is_dir():
            return alt
        raise FileNotFoundError(f"Live2DOutput not created (cwd={cwd})")
    return out


def find_extracted_model_dir(live2d_output: Path, slug: str) -> Path | None:
    # .../Live2DOutput/.../aidang_2/aidang_2/aidang_2.model3.json
    for model3 in live2d_output.rglob(f"{slug}.model3.json"):
        parent = model3.parent
        if (parent / "motions").is_dir():
            return parent
    return None


def merge_slug(extracted: Path, unpacked_slug: Path) -> dict:
    model3_files = list(unpacked_slug.glob("*.model3.json"))
    if not model3_files:
        return {"ok": False, "error": "no model3 in unpacked"}
    model3_path = model3_files[0]
    data = json.loads(model3_path.read_text(encoding="utf-8"))
    src_model3 = json.loads((extracted / f"{unpacked_slug.name}.model3.json").read_text(encoding="utf-8"))
    motions_src = (
        (src_model3.get("FileReferences") or {}).get("Motions")
        if isinstance(src_model3.get("FileReferences"), dict)
        else None
    )
    if not isinstance(motions_src, dict) or not motions_src:
        return {"ok": False, "error": "extracted model3 has no Motions"}

    motions_dir = unpacked_slug / "motions"
    motions_dir.mkdir(exist_ok=True)
    copied = 0
    remapped: dict[str, list[dict]] = {}
    for group, items in motions_src.items():
        if not isinstance(items, list):
            continue
        new_items = []
        for item in items:
            if not isinstance(item, dict):
                continue
            rel = item.get("File")
            if not rel:
                continue
            src = extracted / str(rel).replace("\\", "/")
            if not src.is_file():
                # try basename in motions/
                src = extracted / "motions" / Path(str(rel)).name
            if not src.is_file():
                continue
            dest_name = Path(str(rel)).name
            dest = motions_dir / dest_name
            shutil.copy2(src, dest)
            new_items.append({"File": f"motions/{dest_name}"})
            copied += 1
        if new_items:
            remapped[group] = new_items

    refs = data.setdefault("FileReferences", {})
    refs["Motions"] = remapped
    # Keep existing HitAreas; if missing, leave for config script
    model3_path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return {"ok": True, "motions": copied, "groups": len(remapped)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--bundles",
        type=Path,
        default=repo_root() / "data/model/azurlane/custom",
        help="Folder of Live2D AssetBundles",
    )
    parser.add_argument(
        "--unpacked",
        type=Path,
        default=repo_root() / "data/model/unpacked",
    )
    parser.add_argument("--extractor", type=Path, default=None)
    parser.add_argument(
        "--from-output",
        type=Path,
        default=None,
        help="Reuse existing Live2DOutput (skip running extractor)",
    )
    parser.add_argument("--slug", action="append", default=[])
    args = parser.parse_args()

    if not args.unpacked.is_dir():
        print(json.dumps({"ok": False, "error": f"unpacked missing: {args.unpacked}"}))
        return 1

    live2d_output = args.from_output
    if live2d_output is None:
        exe = find_extractor(args.extractor)
        if not args.bundles.is_dir():
            print(json.dumps({"ok": False, "error": f"bundles missing: {args.bundles}"}))
            return 1
        print(json.dumps({"phase": "extract", "exe": str(exe), "bundles": str(args.bundles)}))
        live2d_output = run_extractor(exe, args.bundles)

    slugs = args.slug or sorted(
        p.name for p in args.unpacked.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    failed = 0
    for slug in slugs:
        dest = args.unpacked / slug
        src = find_extracted_model_dir(live2d_output, slug)
        if not src:
            print(json.dumps({"slug": slug, "ok": False, "error": "not in Live2DOutput"}))
            failed += 1
            continue
        r = merge_slug(src, dest)
        r["slug"] = slug
        r["from"] = str(src)
        print(json.dumps(r, ensure_ascii=False))
        if not r.get("ok"):
            failed += 1
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
