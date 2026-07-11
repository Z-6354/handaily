#!/usr/bin/env python3
"""Find CSS classes used in src but missing from stylesheets."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"


def css_files() -> list[Path]:
    files = [SRC / "styles.css"]
    styles_dir = SRC / "styles"
    if styles_dir.is_dir():
        files.extend(sorted(styles_dir.rglob("*.css")))
    return files


defined: set[str] = set()
for css_path in css_files():
    css = css_path.read_text(encoding="utf-8")
    defined.update(re.findall(r"^\.([a-zA-Z0-9_-]+)", css, re.M))

used: set[str] = set()
for f in SRC.rglob("*"):
    if f.suffix not in (".tsx", ".ts", ".jsx", ".js"):
        continue
    text = f.read_text(encoding="utf-8", errors="ignore")
    for m in re.finditer(r'className="([^"]+)"', text):
        for c in m.group(1).split():
            used.add(c)
    for m in re.finditer(r"className=\{`([^`]+)`\}", text):
        for part in re.split(r"\$\{[^}]+\}", m.group(1)):
            for c in part.split():
                c = c.strip().strip('"').strip("'")
                if c:
                    used.add(c)
    for m in re.finditer(r"className=\{\[([^\]]+)\]\}", text):
        for c in re.findall(r'["\']([^"\']+)["\']', m.group(1)):
            for tok in c.split():
                used.add(tok)


def is_defined(name: str) -> bool:
    if name in defined:
        return True
    base = name.split("--")[0]
    return base in defined


missing = sorted(c for c in used if c and not is_defined(c))
print(f"CSS files scanned: {len(css_files())}")
print(f"Defined selectors: {len(defined)}")
print(f"Used class tokens: {len(used)}")
print(f"Missing: {len(missing)}\n")
for c in missing:
    print(c)
