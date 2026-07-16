"""Native folder / multi-file pickers via tkinter (local serve_web only)."""
from __future__ import annotations

import threading

_DIALOG_LOCK = threading.Lock()


def _ask_directory(title: str) -> str:
    import tkinter as tk
    from tkinter import filedialog

    root = tk.Tk()
    root.withdraw()
    try:
        root.attributes("-topmost", True)
    except tk.TclError:
        pass
    try:
        return filedialog.askdirectory(title=title, mustexist=True) or ""
    finally:
        root.destroy()


def _ask_open_filenames(title: str) -> tuple[str, ...]:
    import tkinter as tk
    from tkinter import filedialog

    root = tk.Tk()
    root.withdraw()
    try:
        root.attributes("-topmost", True)
    except tk.TclError:
        pass
    try:
        result = filedialog.askopenfilenames(title=title)
        if not result:
            return ()
        return tuple(result)
    finally:
        root.destroy()


def pick_folder(title: str = "选择文件夹") -> str | None:
    with _DIALOG_LOCK:
        try:
            path = (_ask_directory(title) or "").strip()
        except Exception as exc:  # noqa: BLE001
            raise OSError(f"无法打开系统对话框：{exc}") from exc
    return path or None


def pick_files(title: str = "选择文件") -> list[str]:
    with _DIALOG_LOCK:
        try:
            paths = _ask_open_filenames(title)
        except Exception as exc:  # noqa: BLE001
            raise OSError(f"无法打开系统对话框：{exc}") from exc
    out: list[str] = []
    seen: set[str] = set()
    for p in paths:
        s = str(p).strip()
        if not s:
            continue
        key = s.lower()
        if key in seen:
            continue
        seen.add(key)
        out.append(s)
    return out
