import type { SettingsFeedback as Feedback } from "../lib/apiErrorMessage";

interface Props {
  feedback?: Feedback | null;
  compact?: boolean;
}

export function SettingsFeedbackBanner({ feedback, compact }: Props) {
  if (!feedback) return null;

  const isSuccess = feedback.tone === "success";

  return (
    <div
      className={`settings-feedback settings-feedback--${feedback.tone}${compact ? " settings-feedback--compact" : ""}${isSuccess ? " settings-feedback--success-layout" : ""}`}
      role={feedback.tone === "error" ? "alert" : "status"}
    >
      <div className="settings-feedback-icon" aria-hidden>
        {feedback.tone === "loading" && "…"}
        {feedback.tone === "success" && "✓"}
        {feedback.tone === "error" && "!"}
        {feedback.tone === "info" && "i"}
      </div>
      <div className="settings-feedback-body">
        <div className="settings-feedback-head">
          <div className="settings-feedback-title">{feedback.title}</div>
          {feedback.tags && feedback.tags.length > 0 && (
            <div className="settings-feedback-tags">
              {feedback.tags.map((tag) => (
                <span key={tag} className="settings-feedback-tag">
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
        {feedback.detail && (
          <div className="settings-feedback-detail">{feedback.detail}</div>
        )}
        {feedback.hint && <div className="settings-feedback-hint">{feedback.hint}</div>}
      </div>
    </div>
  );
}
