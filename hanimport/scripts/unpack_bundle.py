#!/usr/bin/env python3
"""Extract Spine or Live2D Cubism assets from Azur Lane Unity AssetBundles.

Reference workflow: AssetStudioMod + UnityLive2DExtractor (Perfare).
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
from pathlib import Path
from typing import Any

import UnityPy

BUNDLE_MAGIC = b"UnityFS"


def read_bytes(script: str | bytes) -> bytes:
    if isinstance(script, bytes):
        return script
    return script.encode("utf-8", "surrogateescape")


def is_unity_bundle(path: Path) -> bool:
    try:
        with path.open("rb") as f:
            return f.read(len(BUNDLE_MAGIC)) == BUNDLE_MAGIC
    except OSError:
        return False


def infer_slug(path: Path) -> str:
    name = path.name
    if path.suffix:
        name = path.stem
    return name.lower()


def is_spine_atlas(name: str, script: str) -> bool:
    lower = name.lower()
    if lower.endswith(".atlas") or lower.endswith(".atlas.txt"):
        return True
    head = script[:512] if isinstance(script, str) else ""
    return "size:" in head and ("format:" in head or "filter:" in head)


def is_spine_skel(name: str, script: str, blob: bytes) -> bool:
    lower = name.lower()
    if lower.endswith(".skel") or lower.endswith(".skel.bytes"):
        return True
    if lower.endswith(".physics3") or lower.endswith(".json"):
        return False
    if isinstance(script, str) and script.lstrip().startswith("{"):
        return False
    if is_spine_atlas(name, script):
        return False
    return len(blob) > 64


def mono_class_name(obj) -> str | None:
    try:
        data = obj.read()
        script = data.m_Script.read()
        return getattr(script, "m_ClassName", None)
    except Exception:
        return None


def detect_kind(env) -> str:
    has_spine = False
    has_live2d = False
    for obj in env.objects:
        if obj.type.name == "TextAsset":
            txt = obj.read()
            script = txt.m_Script if isinstance(txt.m_Script, str) else ""
            blob = read_bytes(txt.m_Script)
            if is_spine_atlas(txt.m_Name, script) or is_spine_skel(txt.m_Name, script, blob):
                has_spine = True
        elif obj.type.name == "MonoBehaviour":
            if mono_class_name(obj) == "CubismMoc":
                has_live2d = True
    if has_spine:
        return "spine"
    if has_live2d:
        return "live2d"
    return "unknown"


def normalize_atlas_png_line(atlas_text: str, png_name: str) -> str:
    lines = atlas_text.splitlines()
    if not lines:
        return atlas_text
    if lines[0].strip().endswith(".png"):
        lines[0] = png_name
    else:
        lines.insert(0, png_name)
    return "\n".join(lines) + ("\n" if atlas_text.endswith("\n") else "")


def extract_spine(env, out_dir: Path, slug: str) -> list[str]:
    out_dir.mkdir(parents=True, exist_ok=True)
    written: list[str] = []
    atlas_text: str | None = None
    skel_blob: bytes | None = None

    for obj in env.objects:
        if obj.type.name != "TextAsset":
            continue
        txt = obj.read()
        name = txt.m_Name
        script = txt.m_Script if isinstance(txt.m_Script, str) else ""
        blob = read_bytes(txt.m_Script)
        if is_spine_atlas(name, script):
            atlas_text = script if isinstance(script, str) else blob.decode("utf-8", "replace")
        elif is_spine_skel(name, script, blob):
            skel_blob = blob

    textures: list[tuple[str, Any]] = []
    for obj in env.objects:
        if obj.type.name != "Texture2D":
            continue
        tex = obj.read()
        if tex.m_Width and tex.m_Height:
            textures.append((tex.m_Name, tex))

    if not skel_blob and not atlas_text:
        raise ValueError("no Spine TextAsset (.skel / .atlas) found in bundle")

    png_name = f"{slug}.png"
    if atlas_text is not None:
        atlas_path = out_dir / f"{slug}.atlas"
        atlas_path.write_text(normalize_atlas_png_line(atlas_text, png_name), encoding="utf-8")
        written.append(str(atlas_path.name))

    if skel_blob is not None:
        skel_path = out_dir / f"{slug}.skel"
        skel_path.write_bytes(skel_blob)
        written.append(str(skel_path.name))

    if textures:
        # Prefer texture referenced in atlas, else largest area.
        chosen = max(textures, key=lambda t: t[1].m_Width * t[1].m_Height)
        if atlas_text:
            m = re.search(r"^(\S+\.png)\s*$", atlas_text, re.MULTILINE)
            if m:
                ref = m.group(1)
                for tex_name, tex in textures:
                    if tex_name in ref or ref.startswith(tex_name):
                        chosen = (tex_name, tex)
                        break
        tex_path = out_dir / png_name
        chosen[1].image.save(tex_path)
        written.append(str(tex_path.name))

    if not written:
        raise ValueError("Spine bundle parsed but no output files were written")
    return written


def extract_live2d(env, out_dir: Path, slug: str) -> list[str]:
    out_dir.mkdir(parents=True, exist_ok=True)
    written: list[str] = []
    texture_names: list[str] = []

    for obj in env.objects:
        if obj.type.name == "MonoBehaviour" and mono_class_name(obj) == "CubismMoc":
            tree = obj.read_typetree()
            raw = tree.get("_bytes", b"")
            if isinstance(raw, str):
                raw = raw.encode("latin1")
            raw = bytes(raw)
            if raw[:4] != b"MOC3":
                raise ValueError(f"unexpected CubismMoc header for {slug}: {raw[:8]!r}")
            moc_path = out_dir / f"{slug}.moc3"
            moc_path.write_bytes(raw)
            written.append(moc_path.name)

    for obj in env.objects:
        if obj.type.name != "TextAsset":
            continue
        txt = obj.read()
        if not txt.m_Name.lower().endswith(".physics3"):
            continue
        physics_path = out_dir / f"{slug}.physics3.json"
        physics_path.write_bytes(read_bytes(txt.m_Script))
        written.append(physics_path.name)

    for obj in env.objects:
        if obj.type.name != "Texture2D":
            continue
        tex = obj.read()
        if not tex.m_Width:
            continue
        tex_path = out_dir / f"{tex.m_Name}.png"
        tex.image.save(tex_path)
        written.append(tex_path.name)
        texture_names.append(tex_path.name)

    if not any(f.endswith(".moc3") for f in written):
        raise ValueError("no CubismMoc found in Live2D bundle")

    model3 = {
        "Version": 3,
        "FileReferences": {
            "Moc": f"{slug}.moc3",
            "Textures": texture_names,
        },
        "Groups": [],
    }
    physics_file = f"{slug}.physics3.json"
    if physics_file in written:
        model3["FileReferences"]["Physics"] = physics_file

    model_path = out_dir / f"{slug}.model3.json"
    model_path.write_text(json.dumps(model3, indent=2, ensure_ascii=False), encoding="utf-8")
    written.append(model_path.name)
    return written


def unpack_one(input_path: Path, output_root: Path, slug: str | None = None) -> dict[str, Any]:
    if not input_path.is_file():
        raise FileNotFoundError(f"input not found: {input_path}")

    slug = (slug or infer_slug(input_path)).lower()
    out_dir = output_root / slug

    from unpack_complete import is_hx_slug, prepare_unpack_dir

    if is_hx_slug(slug):
        if out_dir.exists():
            shutil.rmtree(out_dir)
        return {
            "slug": slug,
            "kind": "",
            "output_dir": str(out_dir),
            "files": [],
            "skipped": True,
            "skip_reason": "hx",
        }

    if not is_unity_bundle(input_path):
        raise ValueError(f"not a Unity AssetBundle: {input_path}")

    action = prepare_unpack_dir(out_dir, slug)
    if action == "skip":
        files = [p.name for p in out_dir.iterdir() if p.is_file()] if out_dir.is_dir() else []
        kind = "live2d" if (out_dir / f"{slug}.moc3").is_file() else "spine"
        return {
            "slug": slug,
            "kind": kind,
            "output_dir": str(out_dir),
            "files": files,
            "skipped": True,
        }

    env = UnityPy.load(str(input_path))
    kind = detect_kind(env)

    if kind == "spine":
        files = extract_spine(env, out_dir, slug)
    elif kind == "live2d":
        files = extract_live2d(env, out_dir, slug)
    else:
        raise ValueError(
            f"unsupported bundle type for {input_path.name} "
            "(expected Spine .skel/.atlas or Live2D CubismMoc)"
        )

    return {
        "slug": slug,
        "kind": kind,
        "output_dir": str(out_dir),
        "files": files,
        "skipped": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Unpack Azur Lane Unity bundle")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--slug", default=None)
    args = parser.parse_args()

    try:
        result = unpack_one(args.input, args.output, args.slug)
        print(json.dumps({"ok": True, **result}, ensure_ascii=False))
        return 0
    except Exception as exc:  # noqa: BLE001 — CLI boundary
        print(json.dumps({"ok": False, "error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
