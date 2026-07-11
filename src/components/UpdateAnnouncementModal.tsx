import { useEffect, useState } from "react";
import {
  DEFAULT_HELP_CONTENT,
  UPDATE_ANNOUNCEMENT_SEEN_KEY,
  type HelpChangelogEntry,
} from "../lib/helpContent";
import { xiaohan } from "../lib/xiaohan";

interface UpdateAnnouncementModalProps {
  entry: HelpChangelogEntry;
}

export function UpdateAnnouncementModal({ entry }: UpdateAnnouncementModalProps) {
  const [open, setOpen] = useState(false);
  const [dismissing, setDismissing] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const seen = await xiaohan.getSetting(UPDATE_ANNOUNCEMENT_SEEN_KEY);
        if (!cancelled && seen !== "1") {
          setOpen(true);
        }
      } catch {
        if (!cancelled) setOpen(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const dismiss = async () => {
    if (dismissing) return;
    setDismissing(true);
    try {
      await xiaohan.saveSetting(UPDATE_ANNOUNCEMENT_SEEN_KEY, "1");
    } catch {
      // 仍关闭弹窗，避免挡住操作
    } finally {
      setOpen(false);
      setDismissing(false);
    }
  };

  if (!open) return null;

  return (
    <div
      className="update-announcement-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-announcement-title"
    >
      <button
        type="button"
        className="update-announcement-modal__backdrop"
        aria-label="关闭"
        disabled={dismissing}
        onClick={() => void dismiss()}
      />
      <div className="update-announcement-modal__dialog">
        <header className="update-announcement-modal__head">
          <h2 id="update-announcement-title" className="update-announcement-modal__title">
            更新公告
          </h2>
          <div className="update-announcement-modal__meta">
            <span className="update-announcement-modal__ver">v{entry.version}</span>
            {entry.date ? (
              <time className="update-announcement-modal__date">{entry.date}</time>
            ) : null}
          </div>
        </header>
        <p className="update-announcement-modal__body">{entry.body}</p>
        <footer className="update-announcement-modal__actions">
          <button
            type="button"
            className="btn-primary"
            disabled={dismissing}
            onClick={() => void dismiss()}
          >
            知道了
          </button>
        </footer>
      </div>
    </div>
  );
}

export function getLatestChangelogEntry(): HelpChangelogEntry {
  return DEFAULT_HELP_CONTENT.changelog[0] ?? {
    version: "0.2.0",
    date: "",
    body: "欢迎使用小寒桌宠。",
  };
}
