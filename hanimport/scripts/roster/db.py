#!/usr/bin/env python3
"""Handaily roster DB: compatibility re-export hub (impl sheared into roster.* modules).

Commands:
  init | import-wiki | import-bundled-seed | sync-appdata | publish-bundled | export-pack | verify | repair-l2d-binds
"""
from __future__ import annotations


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
import roster.cli as _cli
_pull(_cli)

if __name__ == "__main__":
    raise SystemExit(main())
