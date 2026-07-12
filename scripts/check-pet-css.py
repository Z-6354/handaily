#!/usr/bin/env python3
"""Check pet/main.ts class names against pet.css."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
main_ts = (ROOT / "src" / "pet" / "main.ts").read_text(encoding="utf-8")
pet_css = (ROOT / "src" / "pet" / "pet.css").read_text(encoding="utf-8")
defined = set(re.findall(r"^\.([a-zA-Z0-9_-]+)", pet_css, re.M))

used = set(re.findall(r'className\s*=\s*"([^"]+)"', main_ts))
used |= set(re.findall(r'className\s*=\s*`([^`]+)`', main_ts))
used |= set(re.findall(r'class="([^"]+)"', main_ts))
tokens = set()
for u in used:
    for part in re.split(r"\$\{[^}]+\}", u):
        for t in part.split():
            t = t.strip()
            if t:
                tokens.add(t)

missing = sorted(t for t in tokens if not (t in defined or t.split("--")[0] in defined or t.startswith("is-")))
print("pet.css missing (%d):" % len(missing))
for m in missing:
    print(" ", m)
