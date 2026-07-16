#!/usr/bin/env python3
"""Verify unified skin import for the planned 11 Cubism slugs."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

WANT = [
    "abeikelongbi_3",
    "adaerbote_2",
    "adaerbote_3",
    "aerbien_3",
    "aersasi_2",
    "aersasi_3",
    "aidang_2",
    "aierdeliqi_4",
    "aierdeliqi_5",
    "aijier_2",
    "aijier_3",
]
EXPECTED = {
    "aidang": ("爱宕", "Atago"),
    "adaerbote": ("阿达尔伯特亲王", "Prinz Adalbert"),
    "aersasi": ("阿尔萨斯", "Alsace"),
    "aierdeliqi": ("埃尔德里奇", "Eldridge"),
    "aijier": ("埃吉尔", "Ägir"),
    "abeikelongbi": ("阿贝克隆比", "Abercrombie"),
    "aerbien": ("阿尔比恩", "Albion"),
}


def main() -> int:
    data = Path(os.environ.get("APPDATA", "")) / "xiaohan-daily" / "data"
    manifest_path = data / "characters" / "manifest.json"
    km = data / "kanmusu-models"
    if not manifest_path.is_file():
        print(json.dumps({"ok": False, "error": f"missing {manifest_path}"}))
        return 1
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    by = {c["id"]: c for c in manifest.get("characters", []) if isinstance(c, dict)}
    rows = []
    for folder in WANT:
        base = folder.rsplit("_", 1)[0]
        char = by.get(base)
        skin = next(
            (s for s in (char or {}).get("skins") or [] if s.get("id") == folder),
            None,
        )
        moc = any((km / folder).glob("*.moc3")) if (km / folder).is_dir() else False
        cn_ok = bool(char) and char.get("name") == EXPECTED[base][0]
        en_ok = bool(char) and char.get("english_name") == EXPECTED[base][1]
        lines_n = len((skin or {}).get("lines") or [])
        kd_ok = bool(skin) and skin.get("kanmusu_dir") == folder
        good = cn_ok and en_ok and moc and lines_n > 0 and kd_ok
        rows.append(
            {
                "slug": folder,
                "character_id": base,
                "ok": good,
                "cn_ok": cn_ok,
                "en_ok": en_ok,
                "moc3": moc,
                "lines": lines_n,
                "kanmusu_dir_ok": kd_ok,
                "name": (char or {}).get("name"),
                "english_name": (char or {}).get("english_name"),
                "skin_name": (skin or {}).get("name"),
                "pet_model_id": (skin or {}).get("model_id") or "",
            }
        )
    passed = sum(1 for r in rows if r["ok"])
    out = {"ok": passed == len(WANT), "passed": passed, "total": len(WANT), "rows": rows}
    sys.stdout.buffer.write((json.dumps(out, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    return 0 if out["ok"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
