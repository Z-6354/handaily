import { useCallback, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import {
  xiaohan,
  type SlotPackImportProgress,
  type SlotPackImportResult,
} from "../lib/xiaohan";

interface Props {
  disabled?: boolean;
  onImported: () => void;
  setFeedback: (f: SettingsFeedback | null) => void;
}

export function SlotPackImportButton({ disabled, onImported, setFeedback }: Props) {
  const [importing, setImporting] = useState(false);
  const [progress, setProgress] = useState<SlotPackImportProgress | null>(null);

  const runImport = useCallback(async () => {
    setFeedback(null);
    const picked = await xiaohan.slotPackPickZip();
    if (!picked) return;

    setImporting(true);
    setProgress(null);
    const unlisten = await listen<SlotPackImportProgress>("slot-pack-import-progress", (event) => {
      setProgress(event.payload);
    });

    try {
      const result: SlotPackImportResult = await xiaohan.slotPackImport(picked);
      const failNote =
        result.slotsFailed > 0 ? `，失败 ${result.slotsFailed}` : "";
      setFeedback(
        successFeedback(
          `已导入「${result.packLabel}」`,
          `皮肤槽 ${result.slotsImported}${failNote}；新增 ${result.charactersAdded} 角色，更新 ${result.charactersUpdated}；模型 ${result.modelsCopied}`,
        ),
      );
      onImported();
    } catch (e) {
      setFeedback(parseApiError(e, "导入皮肤包"));
    } finally {
      unlisten();
      setImporting(false);
      setProgress(null);
    }
  }, [onImported, setFeedback]);

  return (
    <div className="roster-pack-import">
      <button
        type="button"
        className="btn-secondary btn-sm"
        disabled={disabled || importing}
        onClick={() => void runImport()}
        title="新版：导入服务器下载的皮肤分发包（多角色 .zip 或单个 .slot.zip）"
      >
        {importing ? "导入中…" : "导入皮肤包（新版）"}
      </button>
      {importing && progress && (
        <p className="roster-pack-import-progress" role="status" aria-live="polite">
          {progress.message}
        </p>
      )}
    </div>
  );
}
