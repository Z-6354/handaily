import { useEffect } from "react";
import { createPortal } from "react-dom";
import type { SettingsFeedback } from "../lib/apiErrorMessage";
import { SettingsFeedbackBanner } from "./SettingsFeedbackBanner";

interface Props {
  feedback: SettingsFeedback | null;
  onDismiss: () => void;
}

function autoDismissMs(tone: SettingsFeedback["tone"]): number | null {
  if (tone === "loading") return null;
  if (tone === "error") return 8000;
  return 4000;
}

export function SettingsFeedbackToast({ feedback, onDismiss }: Props) {
  useEffect(() => {
    if (!feedback) return;
    const delay = autoDismissMs(feedback.tone);
    if (delay == null) return;
    const timer = window.setTimeout(onDismiss, delay);
    return () => window.clearTimeout(timer);
  }, [feedback, onDismiss]);

  if (!feedback) return null;

  return createPortal(
    <div className="settings-feedback-toast-layer">
      <div
        className="settings-feedback-toast"
        role={feedback.tone === "error" ? "alertdialog" : "status"}
        aria-live={feedback.tone === "error" ? "assertive" : "polite"}
      >
        <SettingsFeedbackBanner feedback={feedback} />
        <button
          type="button"
          className="settings-feedback-toast-close"
          aria-label="关闭提示"
          onClick={onDismiss}
        >
          ×
        </button>
      </div>
    </div>,
    document.body,
  );
}
