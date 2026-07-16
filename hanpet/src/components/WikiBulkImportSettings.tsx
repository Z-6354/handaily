import { useState } from "react";

import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";

import { normalizeWikiBulkImportStartResult } from "../lib/wikiBulkImportProgress";

import type { WikiBulkImportContextValue } from "../contexts/WikiBulkImportContext";



interface WikiBulkImportSettingsProps {

  bulk: WikiBulkImportContextValue;

  setFeedback: (f: SettingsFeedback | null) => void;

}



export function WikiBulkImportSettings({ bulk, setFeedback }: WikiBulkImportSettingsProps) {

  const [starting, setStarting] = useState(false);

  const { progress, isActive, start, setOpen } = bulk;



  const runBulkImport = async () => {

    setStarting(true);

    setFeedback(null);

    try {

      const result = normalizeWikiBulkImportStartResult(await start());

      if (result.already_running) {

        setOpen(true);

        setFeedback({

          tone: "info",

          title: "批量导入已在进行中",

          detail: "可在进度弹窗中暂停或停止",

        });

        return;

      }

      if (!result.started) return;

      setFeedback(

        successFeedback("已开始批量导入", "按 BWIKI 角色名爬取，已导入的将自动跳过"),

      );

    } catch (e) {

      setFeedback(parseApiError(e, "批量导入启动"));

    } finally {

      setStarting(false);

    }

  };



  const showProgressBtn = isActive || progress?.phase === "done" || progress?.phase === "error";



  return (
    <>
      <div className="pref-wiki__actions">

        <button

          type="button"

          className="btn-primary"

          disabled={starting || isActive}

          onClick={() => void runBulkImport()}

        >

          {starting ? "启动中…" : isActive ? "导入进行中…" : "全部导入 Wiki 台词"}

        </button>

        {showProgressBtn && (

          <button type="button" className="btn-secondary" onClick={() => setOpen(true)}>

            查看进度

          </button>

        )}

      </div>

    </>

  );

}


