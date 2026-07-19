#!/usr/bin/env python3
"""CLI: pack / unpack handaily-skin-slot zips (local phase ①)."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# scripts/ on path
_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

from common.path_policy import default_pet, default_skin  # noqa: E402
from roster.schema import connect, default_local_db, roster_dir  # noqa: E402
from roster.skin_slot_pack import pack_many, unpack_slot  # noqa: E402


def cmd_pack(args: argparse.Namespace) -> int:
    db = Path(args.db) if args.db else default_local_db()
    out = Path(args.out)
    pet_root = Path(args.pet) if args.pet else default_pet()
    skin_root = Path(args.skin) if args.skin else default_skin()
    avatar_dir = Path(args.avatars) if args.avatars else (roster_dir() / "avatars")
    ids = [x.strip() for x in args.ids.replace(";", ",").split(",") if x.strip()]
    if not ids:
        print("error: --ids required", file=sys.stderr)
        return 2
    conn = connect(db)
    results = pack_many(
        conn,
        ids,
        pet_root=pet_root,
        skin_root=skin_root,
        out_dir=out,
        avatar_dir=avatar_dir if avatar_dir.is_dir() else None,
    )
    conn.close()
    report = []
    for sid, r in zip(ids, results):
        if r.skipped:
            report.append(
                {"skin_id": sid, "ok": False, "code": r.skipped.code, "message": r.skipped.message}
            )
        else:
            report.append({"skin_id": sid, "ok": True, "path": str(r.path)})
    print(json.dumps({"ok": True, "results": report}, ensure_ascii=False, indent=2))
    return 0 if any(r.path for r in results) or not results else 1


def cmd_unpack(args: argparse.Namespace) -> int:
    manifest = unpack_slot(Path(args.zip), dest_root=Path(args.dest))
    print(json.dumps({"ok": True, "manifest": manifest}, ensure_ascii=False, indent=2))
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("pack")
    p.add_argument("--db", type=Path, default=None)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--ids", type=str, required=True, help="comma-separated skin ids")
    p.add_argument("--pet", type=Path, default=None)
    p.add_argument("--skin", type=Path, default=None)
    p.add_argument("--avatars", type=Path, default=None)

    u = sub.add_parser("unpack")
    u.add_argument("--zip", type=Path, required=True)
    u.add_argument("--dest", type=Path, required=True)

    args = ap.parse_args()
    if args.cmd == "pack":
        return cmd_pack(args)
    if args.cmd == "unpack":
        return cmd_unpack(args)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
