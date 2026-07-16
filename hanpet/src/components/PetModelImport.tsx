import type { PetImportStagingPreview, PetModelInfo } from "../lib/xiaohan";

interface Props {
  busy: boolean;
  importModelName: string;
  importStaging: PetImportStagingPreview | null;
  petModelId: string;
  currentModel?: PetModelInfo;
  nameLabel?: string;
  namePlaceholder?: string;
  onModelNameChange: (name: string) => void;
  onPickFolder: () => void;
  onStageFiles: (files: File[]) => void;
  onCommit: () => void;
  onClearStaging: () => void;
  onDeleteModel: () => void;
}

export function PetModelImport({
  busy,
  importModelName,
  importStaging,
  petModelId,
  currentModel,
  nameLabel = "模型名称",
  namePlaceholder = "例如：我的桌宠",
  onModelNameChange,
  onPickFolder,
  onStageFiles,
  onCommit,
  onClearStaging,
  onDeleteModel,
}: Props) {
  return (
    <div className="pet-import-body">
      <div className="pet-dev-shared">
        <label className="pet-dev-shared-label" htmlFor="pet-dev-model-name">
          {nameLabel}
        </label>
        <input
          id="pet-dev-model-name"
          className="pet-dev-input"
          placeholder={namePlaceholder}
          value={importModelName}
          disabled={busy}
          onChange={(e) => onModelNameChange(e.target.value)}
        />
        <p className="pet-dev-shared-hint">
          先选择文件夹或文件完成本地缓存，确认无误后点击「开始导入」
        </p>
      </div>

      <div className="pet-dev-blocks">
        <section className="pet-dev-block">
          <div className="pet-dev-block-head">
            <span className="pet-dev-badge pet-dev-badge--folder">文件夹</span>
            <div>
              <h4 className="pet-dev-block-title">从文件夹导入</h4>
              <p className="pet-dev-block-desc">
                选择含 Spine 三件套的文件夹；无配置文件时自动生成 config.json
              </p>
            </div>
          </div>
          <button
            type="button"
            className="btn-secondary btn-sm pet-dev-pick-btn"
            disabled={busy}
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onPickFolder();
            }}
          >
            选择文件夹
          </button>
        </section>

        <div className="pet-lines-import-divider" aria-hidden>
          <span>或</span>
        </div>

        <section className="pet-dev-block">
          <div className="pet-dev-block-head">
            <span className="pet-dev-badge pet-dev-badge--files">文件</span>
            <div>
              <h4 className="pet-dev-block-title">从文件导入</h4>
              <p className="pet-dev-block-desc">一次选择三个 Spine 资源文件并缓存到本地</p>
            </div>
          </div>
          <label className="pet-dev-file-picker">
            <input
              type="file"
              accept=".skel,.atlas,.png"
              multiple
              hidden
              disabled={busy}
              onChange={(e) => {
                const files = Array.from(e.target.files ?? []);
                e.target.value = "";
                onStageFiles(files);
              }}
            />
            <span className="pet-dev-file-picker-icon" aria-hidden>
              +
            </span>
            <span className="pet-dev-file-picker-text">选择 Spine 文件</span>
          </label>
        </section>
      </div>

      {importStaging && (
        <section className="pet-dev-staging">
          <div className="pet-dev-staging-head">
            <h4 className="pet-dev-block-title">已缓存，待导入</h4>
            <button
              type="button"
              className="btn-secondary btn-sm"
              disabled={busy}
              onClick={onClearStaging}
            >
              清除
            </button>
          </div>
          <ul className="pet-dev-staging-list">
            <li>
              来源：{importStaging.source === "folder" ? "文件夹" : "文件"}
              {importStaging.folder_path ? ` · ${importStaging.folder_path}` : ""}
            </li>
            <li>
              文件：{importStaging.skel_file} · {importStaging.atlas_file} ·{" "}
              {importStaging.png_file}
            </li>
            <li>
              配置：{importStaging.config_file ?? "无"}
              {importStaging.config_generated ? "（已自动生成）" : "（已缓存）"}
            </li>
          </ul>
        </section>
      )}

      <div className="pet-dev-commit-row">
        <button
          type="button"
          className="btn-primary pet-dev-commit-btn"
          disabled={busy || !importModelName.trim() || !importStaging}
          onClick={onCommit}
        >
          开始导入
        </button>
      </div>

      {petModelId !== "chaijun" && (
        <section className="pet-dev-danger">
          <div className="pet-dev-block-head">
            <span className="pet-dev-badge pet-dev-badge--danger">删除</span>
            <div>
              <h4 className="pet-dev-block-title">删除当前模型</h4>
              <p className="pet-dev-block-desc">仅可删除用户导入的模型；内置柴郡不可删除</p>
            </div>
          </div>
          <div className="pet-dev-danger-row">
            <span className="pet-dev-danger-target">{currentModel?.name ?? petModelId}</span>
            <button
              type="button"
              className="btn-secondary btn-sm pet-dev-danger-btn"
              disabled={busy}
              onClick={onDeleteModel}
            >
              删除模型
            </button>
          </div>
        </section>
      )}
    </div>
  );
}
