import { useEffect, useId, useRef, useState } from "react";
import type { PetModelInfo } from "../lib/xiaohan";

interface Props {
  models: PetModelInfo[];
  activeId: string;
  switchingId: string | null;
  disabled?: boolean;
  onSelect: (id: string) => void;
  layout?: "grid" | "compact";
}

function ModelCard({
  model,
  active,
  switching,
  disabled,
  onSelect,
}: {
  model: PetModelInfo;
  active: boolean;
  switching: boolean;
  disabled?: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      className={`pet-model-card${active ? " is-active" : ""}${switching ? " is-switching" : ""}`}
      disabled={disabled || switching}
      aria-pressed={active}
      aria-busy={switching}
      onClick={onSelect}
    >
      <span className="pet-model-card-name">{model.name}</span>
      <span className={`pet-model-card-badge${model.builtin ? " is-builtin" : ""}`}>
        {model.builtin ? "内置" : "导入"}
      </span>
      {switching && <span className="pet-model-card-spinner" aria-hidden />}
    </button>
  );
}

export function PetModelPicker({
  models,
  activeId,
  switchingId,
  disabled,
  onSelect,
  layout = "grid",
}: Props) {
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const active = models.find((m) => m.id === activeId);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const pick = (id: string) => {
    if (id === activeId || switchingId || disabled) return;
    onSelect(id);
    setOpen(false);
  };

  const cards = (
    <div className="pet-model-grid" role="listbox" id={listId} aria-label="桌宠模型">
      {models.map((m) => (
        <ModelCard
          key={m.id}
          model={m}
          active={m.id === activeId}
          switching={m.id === switchingId}
          disabled={disabled || Boolean(switchingId)}
          onSelect={() => pick(m.id)}
        />
      ))}
    </div>
  );

  if (layout === "compact") {
    return (
      <div className="pet-model-picker-compact" ref={rootRef}>
        <button
          type="button"
          className={`pet-model-picker-trigger${open ? " is-open" : ""}${switchingId ? " is-busy" : ""}`}
          disabled={disabled || models.length === 0}
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-controls={listId}
          onClick={() => setOpen((v) => !v)}
        >
          <span className="pet-model-picker-trigger-label">模型</span>
          <span className="pet-model-picker-trigger-name">
            {switchingId ? "切换中…" : (active?.name ?? "未选择")}
          </span>
          {switchingId && <span className="pet-model-card-spinner" aria-hidden />}
          <span className="pet-model-picker-chevron" aria-hidden />
        </button>
        {open && <div className="pet-model-picker-popover">{cards}</div>}
      </div>
    );
  }

  return (
    <section className="pet-model-section" aria-labelledby={`${listId}-label`}>
      <div className="pet-model-section-head" id={`${listId}-label`}>
        <span className="pet-model-section-title">选择模型</span>
        {switchingId && (
          <span className="pet-model-section-hint">正在加载，桌宠窗口将短暂刷新…</span>
        )}
      </div>
      {cards}
    </section>
  );
}
