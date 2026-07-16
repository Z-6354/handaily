# Folder-like character purge — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Purge character rows whose ids are skin folders (`abeikelongbi_3`), merge into base, guard upsert/API.

**Tech Stack:** Python, pytest, existing `strip_skin` / `_merge_character_into`

---

### Task 1: `is_folder_like_character_id` + purge (TDD)

**Files:** `test_purge_folder_chars.py`, `roster_db.py`

### Task 2: Guard upsert + API create

**Files:** `roster_db.py`, `roster_api.py`, tests

### Task 3: Wire wiki pipeline + run purge on local DB

**Files:** `wiki_pipeline_jobs.py`; one-shot purge of local roster
