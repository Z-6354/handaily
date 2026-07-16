# Unpack skip + purge `*_hx` Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Skip unpacking slugs ending in `_hx`, log `跳过(hx)`, and delete existing `*_hx` dirs under the job output root.

**Architecture:** Shared helpers in `unpack_complete.py`; Job path in `serve_web.py` filters/logs; `unpack_bundle.unpack_one` as CLI safety net.

**Tech Stack:** Python 3, pytest

---

### Task 1: `is_hx_slug` + purge helper (TDD)

**Files:**
- Create: `hanimport/scripts/test_unpack_skip_hx.py`
- Modify: `hanimport/scripts/unpack_complete.py`

Steps: failing tests → implement `is_hx_slug` / `purge_hx_output_dirs` → green.

### Task 2: `unpack_one` skip hx

**Files:**
- Modify: `hanimport/scripts/unpack_bundle.py`
- Modify: `hanimport/scripts/test_unpack_skip_hx.py`

Steps: test unpack_one returns skipped hx and removes out dir → implement.

### Task 3: Job path log + purge

**Files:**
- Modify: `hanimport/scripts/serve_web.py`
- Modify: `hanimport/scripts/test_unpack_skip_hx.py` or `test_serve_jobs.py`

Steps: hx bundles log `跳过(hx)` and do not call unpack; purge under output_root.
