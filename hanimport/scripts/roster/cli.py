"""Roster DB CLI entry (sheared from db.py C1)."""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import os
import re
import shutil
import sqlite3
import sys
import zipfile
from pathlib import Path


def _pull(mod) -> None:
    g = globals()
    for k, v in vars(mod).items():
        # Never copy _pull itself — its globals() is bound to the defining module.
        if k.startswith("__") or k == "_pull":
            continue
        g[k] = v

import roster.ids as _ids
_pull(_ids)

import roster.schema as _schema
_pull(_schema)

import roster.crud as _crud
_pull(_crud)

import roster.merge as _merge
_pull(_merge)

import roster.bind_pipeline as _bind_pipeline
_pull(_bind_pipeline)

import roster.import_wiki as _import_wiki
_pull(_import_wiki)

import roster.sync as _sync
_pull(_sync)

# --- cli ---

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--db", type=Path, default=None)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_init = sub.add_parser("init")
    p_init.add_argument("--force", action="store_true")

    p_seed = sub.add_parser("import-bundled-seed")

    p_imp = sub.add_parser("import-wiki")
    p_imp.add_argument(
        "--wiki-db",
        type=Path,
        default=None,
        help="BWIKI sqlite (default: path_policy.default_wiki_db())",
    )
    p_imp.add_argument(
        "--unpacked",
        type=Path,
        default=repo_root() / "data/skin",
    )
    p_imp.add_argument(
        "--en-map",
        type=Path,
        default=repo_root() / "data/wiki/ship-en-names.json",
    )
    p_imp.add_argument(
        "--ids",
        type=str,
        default="",
        help="仅同步这些角色 id（逗号分隔），默认全部",
    )
    p_imp.add_argument(
        "--scope",
        choices=("all", "unpacked"),
        default="all",
        help="all=Wiki 全舰船；unpacked=仅已解包目录",
    )

    p_sync = sub.add_parser("sync-appdata")
    p_sync.add_argument("--data-dir", type=Path, default=None)
    p_sync.add_argument("--ids", type=str, default="")
    p_sync.add_argument("--force-lines", action="store_true")
    p_sync.add_argument(
        "--merge",
        action="store_true",
        help="合并进 AppData 现有角色（默认改为覆盖：仅保留本次同步的自用库角色）",
    )

    p_pub = sub.add_parser("publish-bundled")
    p_pub.add_argument("--ids", type=str, default="", help="override allowlist")

    p_pack = sub.add_parser("export-pack")
    p_pack.add_argument("--ids", type=str, required=True)
    p_pack.add_argument("-o", "--output", type=Path, required=True)

    sub.add_parser("verify")
    sub.add_parser(
        "repair-l2d-binds",
        help="Merge leftover L2D-{N} into Wiki skin{N-1} (slug_N folder convention)",
    )

    args = ap.parse_args()
    if args.cmd == "init":
        return cmd_init(args)
    if args.cmd == "import-bundled-seed":
        return cmd_import_bundled_seed(args)
    if args.cmd == "import-wiki":
        return cmd_import_wiki(args)
    if args.cmd == "sync-appdata":
        return cmd_sync_appdata(args)
    if args.cmd == "publish-bundled":
        return cmd_publish_bundled(args)
    if args.cmd == "export-pack":
        return cmd_export_pack(args)
    if args.cmd == "verify":
        return cmd_verify(args)
    if args.cmd == "repair-l2d-binds":
        return cmd_repair_l2d_binds(args)
    return 1

