import { useCallback, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import {
  xiaohan,
  type RosterPackImportProgress,
  type RosterPackImportResult,
} from "../lib/xiaohan";

interface Props {
  disabled?: boolean;
  onImported: () => void;
  setFeedback: (f: SettingsFeedback | null) => void;
}

export function RosterPackImportButton({ disabled, onImported, setFeedback }: Props) {
  const [importing, setImporting] = useState(false);
  const [progress, setProgress] = useState<RosterPackImportProgress | null>(null);

  const runImport = useCallback(async () => {
    setFeedback(null);
    const picked = await xiaohan.rosterPackPickZip();
    if (!picked) return;

    setImporting(true);
    setProgress(null);
    const unlisten = await listen<RosterPackImportProgress>("roster-pack-import-progress", (event) => {
      setProgress(event.payload);
    });

    try {
      const result: RosterPackImportResult = await xiaohan.rosterPackImport(picked);
      setFeedback(
        successFeedback(
          `已导入「${result.packLabel}」`,
          `新增 ${result.charactersAdded} 角色，更新 ${result.charactersUpdated}；合并 ${result.modelsCopied} 模型（跳过 ${result.modelsSkipped} 内置）`,
        ),
      );
      onImported();
    } catch (e) {
      setFeedback(parseApiError(e, "导入角色包"));
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
        title="旧版：导入模型-xx阵营角色包、模型-其他角色包、模型-柴郡角色包等 zip"
      >
        {importing ? "导入中…" : "导入角色包（旧版）"}
      </button>
      {importing && progress && (
        <p className="roster-pack-import-progress" role="status" aria-live="polite">
          {progress.message}
        </p>
      )}
    </div>
  );
}
