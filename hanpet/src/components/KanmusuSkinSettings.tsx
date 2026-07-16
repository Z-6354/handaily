import { useState } from "react";
import {
  parseApiError,
  successFeedback,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import { xiaohan } from "../lib/xiaohan";

interface Props {
  setFeedback: (f: SettingsFeedback | null) => void;
}

/** 设置页：与「台词导入」并列的舰娘皮肤同步入口 */
export function KanmusuSkinSettings({ setFeedback }: Props) {
  const [busy, setBusy] = useState(false);

  const sync = async () => {
    setBusy(true);
    setFeedback(null);
    try {
      const result = await xiaohan.kanmusuSyncFromUnpacked();
      setFeedback(
        successFeedback(
          result.message || "已同步舰娘模型",
          `新增角色 ${result.added_characters} · 皮肤 ${result.added_skins} · 共 ${result.synced_slugs.length} 个目录`,
        ),
      );
    } catch (e) {
      setFeedback(parseApiError(e, "同步舰娘"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <p className="hint-block">
        从仓库 <code>data/model/unpacked</code> 同步 Cubism 到本机，并挂到已有人物皮肤（按目录名序号对齐）。
      </p>
      <div className="pref-wiki__actions">
        <button
          type="button"
          className="btn-primary"
          disabled={busy}
          onClick={() => void sync()}
        >
          {busy ? "同步中…" : "同步舰娘皮肤"}
        </button>
      </div>
    </>
  );
}
