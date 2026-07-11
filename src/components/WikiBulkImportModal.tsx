import type { PetWikiBulkImportProgress } from "../lib/xiaohan";

interface WikiBulkImportModalProps {
  open: boolean;
  progress: PetWikiBulkImportProgress | null;
  isActive: boolean;
  isPaused: boolean;
  onPause: () => void;
  onResume: () => void;
  onStop: () => void;
  onDismiss: () => void;
}

export function WikiBulkImportModal({
  open,
  progress,
  isActive,
  isPaused,
  onPause,
  onResume,
  onStop,
  onDismiss,
}: WikiBulkImportModalProps) {
  if (!open || !progress) return null;

  const { phase, index, total, model_name, message, succeeded, failed, skipped } = progress;
  const isError = phase === "error";
  const isDone = phase === "done";
  const isIndeterminate = isActive && phase === "scan" && index === 0 && total === 0;
  const pct =
    isDone
      ? 100
      : total > 0
        ? Math.max(4, Math.min(100, Math.round((index / total) * 100)))
        : isActive
          ? 8
          : 0;

  const title = isError
    ? "Wiki 台词导入结束（含失败）"
    : isDone
      ? "Wiki 台词导入完成"
      : isPaused
        ? "Wiki 台词导入已暂停"
        : "Wiki 台词批量导入";

  return (
    <div className="wiki-bulk-modal" role="dialog" aria-modal="true" aria-labelledby="wiki-bulk-modal-title">
      <button
        type="button"
        className="wiki-bulk-modal__backdrop"
        aria-label="关闭"
        onClick={isActive ? undefined : onDismiss}
        tabIndex={isActive ? -1 : 0}
      />
      <div
        className={`wiki-bulk-modal__dialog${
          isError
            ? " wiki-bulk-modal__dialog--error"
            : isActive
              ? " wiki-bulk-modal__dialog--active"
              : " wiki-bulk-modal__dialog--done"
        }`}
      >
        <div className="wiki-bulk-modal__main">
          <div className="wiki-bulk-modal__head">
            <div className="wiki-bulk-modal__icon" aria-hidden>
              {isActive ? (
                <span className="wiki-bulk-modal__spinner" />
              ) : isError ? (
                <span className="wiki-bulk-modal__mark">!</span>
              ) : (
                <span className="wiki-bulk-modal__mark">✓</span>
              )}
            </div>
            <div className="wiki-bulk-modal__copy">
              <h2 id="wiki-bulk-modal-title" className="wiki-bulk-modal__title">
                {title}
              </h2>
              <p className="wiki-bulk-modal__message">
                {phase === "import" && model_name ? (
                  <>
                    <span className="wiki-bulk-modal__model">{model_name}</span>
                    <span className="wiki-bulk-modal__sep"> · </span>
                    {message}
                  </>
                ) : (
                  message
                )}
              </p>
            </div>
            {total > 0 && (
              <div className="wiki-bulk-modal__counter" aria-label={`进度 ${index} / ${total}`}>
                <span className="wiki-bulk-modal__counter-current">{index}</span>
                <span className="wiki-bulk-modal__counter-sep">/</span>
                <span className="wiki-bulk-modal__counter-total">{total}</span>
              </div>
            )}
          </div>

          <div
            className={`wiki-bulk-modal__track${isIndeterminate ? " wiki-bulk-modal__track--indeterminate" : ""}`}
            aria-hidden
          >
            <div
              className="wiki-bulk-modal__fill"
              style={isIndeterminate ? undefined : { width: `${pct}%` }}
            />
          </div>

          {(phase === "scan" || phase === "import" || phase === "done" || phase === "error") && (
            <div className="wiki-bulk-modal__stats">
              {phase === "import" || phase === "done" || phase === "error" ? (
                <>
                  <span className="wiki-bulk-modal__stat wiki-bulk-modal__stat--ok">
                    成功 {succeeded}
                  </span>
                  <span className="wiki-bulk-modal__stat">跳过 {skipped}</span>
                  {failed > 0 && (
                    <span className="wiki-bulk-modal__stat wiki-bulk-modal__stat--fail">
                      失败 {failed}
                    </span>
                  )}
                </>
              ) : (
                <span className="wiki-bulk-modal__stat">
                  已扫描 {index}/{total} 个角色
                </span>
              )}
            </div>
          )}
        </div>

        <aside className="wiki-bulk-modal__actions">
          {isActive ? (
            <>
              <button
                type="button"
                className="btn-secondary wiki-bulk-modal__action"
                onClick={isPaused ? onResume : onPause}
              >
                {isPaused ? "继续" : "暂停"}
              </button>
              <button
                type="button"
                className="btn-secondary wiki-bulk-modal__action wiki-bulk-modal__action--stop"
                onClick={onStop}
              >
                停止
              </button>
            </>
          ) : (
            <button
              type="button"
              className="btn-primary wiki-bulk-modal__action"
              onClick={onDismiss}
            >
              关闭
            </button>
          )}
        </aside>
      </div>
    </div>
  );
}
