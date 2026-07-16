# hanimport CSS Layer Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize hanimport web CSS into base → components → layout → pages with no cross-page CSS deps, plus light Apple-style polish.

**Architecture:** Shared surface/controls live in `components.css`; each HTML page links exactly one `pages/*.css`. Update `serve_web.py` routes; delete obsolete root CSS files.

**Tech Stack:** Plain CSS + static HTML/JS; Python `serve_web.py` static map.

## Global Constraints

- Do not change API / job / roster business semantics
- No SPA, no bundler, no dark mode
- Minimal HTML class renames only for page header unification

---

### Task 1: Expand components + shell; create pages/

**Files:**
- Modify: `hanimport/web/components.css`, `hanimport/web/shell.css`, `hanimport/web/design-system/tokens.css`
- Create: `hanimport/web/pages/{hub,unpack,roster,skins}.css`

- [x] Move into `components.css` from `style.css` / `roster.css`: ghost/secondary buttons, banner, progress, card padding defaults, labels, path-row, actions, form, panel*, item-list, skin-status-table, pager, log, empty-state surface helpers
- [x] Create four page CSS files with only page-specific rules
- [x] Optionally add `--radius-xl` / spacing tokens; hub cards use tokens
- [x] Wire HTML `<link>` to `/pages/*.css`; unify headers to `.page-title` / `.page-sub`
- [x] Update `serve_web.py` static routes; remove old CSS files
- [x] Smoke: open pages or grep that no HTML references `/style.css` or `/roster.css` cross-link from skins

**Verify:** skins.html does not link roster.css; unpack does not need root style.css; serve routes resolve all `/pages/*.css`.
