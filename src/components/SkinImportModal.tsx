import { useCallback, useEffect, useState } from "react";
import { PetModelImport } from "./PetModelImport";
import { SkinDeleteModal } from "./SkinDeleteModal";
import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan, type PetImportStagingPreview } from "../lib/xiaohan";

type Props = {
  open: boolean;
  characterId: string;
  modelId: string;
  modelName?: string;
  onClose: () => void;
  onImported: () => void | Promise<void>;
  setFeedback: (f: SettingsFeedback | null) => void;
};

export function SkinImportModal({
  open,
  characterId,
  modelId,
  modelName,
  onClose,
  onImported,
  setFeedback,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [importModelName, setImportModelName] = useState("");
  const [importStaging, setImportStaging] = useState<PetImportStagingPreview | null>(null);

  useEffect(() => {
    if (!open) return;
    void xiaohan
      .petGetImportStaging()
      .then(setImportStaging)
      .catch(() => setImportStaging(null));
  }, [open]);

  const fileToBase64 = (file: File) =>
    new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        const result = reader.result;
        if (typeof result !== "string") {
          reject(new Error("读取文件失败"));
          return;
        }
        const comma = result.indexOf(",");
        resolve(comma >= 0 ? result.slice(comma + 1) : result);
      };
      reader.onerror = () => reject(reader.error ?? new Error("读取文件失败"));
      reader.readAsDataURL(file);
    });

  const runPickFolder = async () => {
    setFeedback(null);
    try {
      const folder = await xiaohan.petPickModelFolder();
      if (!folder) return;
      setBusy(true);
      const preview = await xiaohan.petStageFolderImport(folder);
      setImportStaging(preview);
      setFeedback({
        tone: "success",
        title: "已缓存文件夹",
        detail: preview.config_generated
          ? "已检测到 Spine 三件套，并自动生成 config.json"
          : `已检测到 Spine 三件套，并缓存配置文件 ${preview.config_file ?? ""}`,
      });
    } catch (e) {
      setFeedback(parseApiError(e, "选择文件夹"));
    } finally {
      setBusy(false);
    }
  };

  const runStageFiles = async (files: File[]) => {
    if (files.length < 3) {
      setFeedback({
        tone: "error",
        title: "选择文件失败",
        detail: "请一次选择 .skel、.atlas、.png 三个文件",
      });
      return;
    }
    const skel = files.find((f) => f.name.toLowerCase().endsWith(".skel"));
    const atlas = files.find((f) => f.name.toLowerCase().endsWith(".atlas"));
    const png = files.find((f) => f.name.toLowerCase().endsWith(".png"));
    if (!skel || !atlas || !png) {
      setFeedback({
        tone: "error",
        title: "选择文件失败",
        detail: "缺少 .skel / .atlas / .png 之一",
      });
      return;
    }
    setBusy(true);
    setFeedback(null);
    try {
      const [skel_b64, atlas_b64, png_b64] = await Promise.all([
        fileToBase64(skel),
        fileToBase64(atlas),
        fileToBase64(png),
      ]);
      const preview = await xiaohan.petStageFilesImport({
        skel_b64,
        atlas_b64,
        png_b64,
        skel_name: skel.name,
        atlas_name: atlas.name,
        png_name: png.name,
      });
      setImportStaging(preview);
      setFeedback({
        tone: "success",
        title: "已缓存文件",
        detail: "三件套已写入本地缓存，点击「开始导入」完成导入",
      });
    } catch (e) {
      setFeedback(parseApiError(e, "缓存文件"));
    } finally {
      setBusy(false);
    }
  };

  const runCommitImport = async () => {
    const name = importModelName.trim();
    if (!name) {
      setFeedback({ tone: "error", title: "无法导入", detail: "请先填写皮肤名称" });
      return;
    }
    if (!importStaging) {
      setFeedback({ tone: "error", title: "无法导入", detail: "请先选择文件夹或文件并完成缓存" });
      return;
    }
    setBusy(true);
    setFeedback(null);
    try {
      await xiaohan.petCommitImport(name, characterId);
      const status = await xiaohan.petGetStatus();
      if (!status.enabled) await xiaohan.petSetEnabled(true);
      setImportModelName("");
      setImportStaging(null);
      await onImported();
      setFeedback(successFeedback(`已导入皮肤「${name}」`));
      onClose();
    } catch (e) {
      setFeedback(parseApiError(e, "导入皮肤"));
    } finally {
      setBusy(false);
    }
  };

  const runClearStaging = async () => {
    setBusy(true);
    setFeedback(null);
    try {
      await xiaohan.petClearImportStaging();
      setImportStaging(null);
    } catch (e) {
      setFeedback(parseApiError(e, "清除缓存"));
    } finally {
      setBusy(false);
    }
  };

  const runDeleteModel = async () => {
    const isBuiltin = ["chaijun", "edu", "wushiling", "qiye", "tashigan"].includes(modelId);
    if (isBuiltin) return;
    setBusy(true);
    setFeedback(null);
    try {
      await xiaohan.petDeleteModel(modelId);
      await onImported();
      setFeedback(successFeedback("已删除模型"));
      setDeleteOpen(false);
      onClose();
    } catch (e) {
      setFeedback(parseApiError(e, "删除模型"));
    } finally {
      setBusy(false);
    }
  };

  const handleClose = useCallback(() => {
    if (busy) return;
    onClose();
  }, [busy, onClose]);

  if (!open) return null;

  const isBuiltin = ["chaijun", "edu", "wushiling", "qiye", "tashigan"].includes(modelId);
  const currentModel = { id: modelId, name: modelName ?? modelId, builtin: isBuiltin };

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-dialog skin-import-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3 className="modal-title">导入皮肤模型</h3>
          <button
            type="button"
            className="modal-close"
            onClick={handleClose}
            disabled={busy}
            aria-label="关闭"
          >
            ×
          </button>
        </div>
        <div className="skin-import-modal-body">
          <p className="hint-block">
            导入 Spine 三件套后将作为<strong>新皮肤</strong>挂到当前人物。
          </p>
          <PetModelImport
            busy={busy}
            importModelName={importModelName}
            importStaging={importStaging}
            petModelId={modelId}
            currentModel={currentModel}
            nameLabel="皮肤名称"
            namePlaceholder="例如：夏日皮肤"
            onModelNameChange={setImportModelName}
            onPickFolder={() => void runPickFolder()}
            onStageFiles={(files) => void runStageFiles(files)}
            onCommit={() => void runCommitImport()}
            onClearStaging={() => void runClearStaging()}
            onDeleteModel={() => setDeleteOpen(true)}
          />
        </div>
      </div>
      <SkinDeleteModal
        open={deleteOpen}
        skinName={modelName ?? modelId}
        deleting={busy}
        onClose={() => {
          if (!busy) setDeleteOpen(false);
        }}
        onConfirm={runDeleteModel}
      />
    </div>
  );
}
