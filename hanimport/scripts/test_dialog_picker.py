"""TDD: native folder/file dialog wrappers."""
from __future__ import annotations

from dialog_picker import pick_files, pick_folder


def test_pick_folder_returns_path(monkeypatch):
    monkeypatch.setattr("dialog_picker._ask_directory", lambda title: r"D:\models\custom")
    assert pick_folder("选目录") == r"D:\models\custom"


def test_pick_folder_cancelled(monkeypatch):
    monkeypatch.setattr("dialog_picker._ask_directory", lambda title: "")
    assert pick_folder() is None


def test_pick_files_returns_list(monkeypatch):
    monkeypatch.setattr(
        "dialog_picker._ask_open_filenames",
        lambda title: (r"D:\a.ab", r"D:\b.ab"),
    )
    assert pick_files() == [r"D:\a.ab", r"D:\b.ab"]


def test_pick_files_cancelled(monkeypatch):
    monkeypatch.setattr("dialog_picker._ask_open_filenames", lambda title: ())
    assert pick_files() == []


def test_pick_folder_raises_readable(monkeypatch):
    def boom(_title):
        raise RuntimeError("no display")

    monkeypatch.setattr("dialog_picker._ask_directory", boom)
    try:
        pick_folder()
        assert False, "expected OSError"
    except OSError as exc:
        assert "无法打开系统对话框" in str(exc)
        assert "no display" in str(exc)
