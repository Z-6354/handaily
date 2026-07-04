import { useMemo, useState } from "react";
import { PersonaEditForm } from "./PersonaEditForm";
import { PersonaTextImportForm } from "./PersonaTextImportForm";
import type { CharacterProfileData, PersonaDetail, PersonaInfo } from "../lib/xiaohan";

type Tab = "skill" | "profile" | "json" | "import" | "edit";

const PERSONA_ACCENT: Record<string, string> = {
  default: "#64748b",
  cheshire: "#f59e0b",
  phoebe: "#a78bfa",
  sora: "#94a3b8",
};

type Props = {
  detail: PersonaDetail | null;
  loading: boolean;
  personas: PersonaInfo[];
  onSelectPersona: (id: string) => void;
  onActivate: (id: string) => void;
  onBack: () => void;
  onUpdated: () => void | Promise<void>;
};

function personaInitial(name: string) {
  const t = name.trim();
  return t ? t.charAt(0) : "?";
}

function FieldBlock({ label, children }: { label: string; children: React.ReactNode }) {
  if (!children || (typeof children === "string" && !children.trim())) return null;
  return (
    <div className="persona-field-block">
      <div className="persona-field-label">{label}</div>
      <div className="persona-field-value">{children}</div>
    </div>
  );
}

function StructuredProfile({ data }: { data: CharacterProfileData }) {
  return (
    <div className="persona-structured">
      <FieldBlock label="介绍">{data.introduction}</FieldBlock>
      <FieldBlock label="说话风格">{data.speech_style}</FieldBlock>
      {data.personality.length > 0 && (
        <FieldBlock label="性格">
          <ul className="persona-tag-list">
            {data.personality.map((t) => (
              <li key={t}>{t}</li>
            ))}
          </ul>
        </FieldBlock>
      )}
      {data.sample_lines.length > 0 && (
        <FieldBlock label="台词示例">
          <ul className="persona-lines-list">
            {data.sample_lines.map((t) => (
              <li key={t}>「{t}」</li>
            ))}
          </ul>
        </FieldBlock>
      )}
      <FieldBlock label="关系">{data.relationships}</FieldBlock>
      {data.taboos.length > 0 && (
        <FieldBlock label="禁忌">
          <ul className="persona-tag-list persona-tag-list--muted">
            {data.taboos.map((t) => (
              <li key={t}>{t}</li>
            ))}
          </ul>
        </FieldBlock>
      )}
      {Object.keys(data.extra).length > 0 && (
        <FieldBlock label="其它">
          <dl className="persona-extra-dl">
            {Object.entries(data.extra).map(([k, v]) => (
              <div key={k} className="persona-extra-row">
                <dt>{k}</dt>
                <dd>{v}</dd>
              </div>
            ))}
          </dl>
        </FieldBlock>
      )}
    </div>
  );
}

export function PersonaDetailPanel({
  detail,
  loading,
  personas,
  onSelectPersona,
  onActivate,
  onBack,
  onUpdated,
}: Props) {
  const [tab, setTab] = useState<Tab>("skill");

  const accent = detail ? (PERSONA_ACCENT[detail.id] ?? "#722ed1") : "#722ed1";
  const jsonText = useMemo(
    () => (detail ? JSON.stringify(detail.profile_json, null, 2) : ""),
    [detail],
  );

  return (
    <div
      className="persona-detail-shell"
      style={{ "--persona-accent": accent } as React.CSSProperties}
    >
      <div className="persona-detail-top">
        <button type="button" className="persona-detail-back" onClick={onBack}>
          ← 返回选角
        </button>
        {personas.length > 1 && detail && (
          <select
            className="persona-detail-switcher"
            value={detail.id}
            onChange={(e) => onSelectPersona(e.target.value)}
          >
            {personas.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        )}
        {detail && !detail.active && (
          <button
            type="button"
            className="btn-primary btn-sm"
            onClick={() => onActivate(detail.id)}
          >
            选用此人设
          </button>
        )}
      </div>

      {loading && !detail ? (
        <p className="hint-block persona-detail-loading">加载详情…</p>
      ) : detail ? (
        <div className="persona-detail-layout">
          <aside className="persona-detail-side">
            <div className="persona-detail-side-cover" />
            <div className="persona-detail-side-avatar" aria-hidden>
              {personaInitial(detail.name)}
            </div>
            <h3 className="persona-detail-side-name">{detail.name}</h3>
            {detail.source && <span className="persona-detail-side-chip">{detail.source}</span>}
            <p className="persona-detail-side-desc">{detail.description}</p>
            <div className="persona-detail-badges">
              {detail.active && <span className="persona-badge persona-badge--active">当前使用</span>}
              {detail.is_builtin && <span className="persona-badge">内置</span>}
            </div>
          </aside>

          <section className="persona-detail-main">
            <div className="persona-detail-tabs">
              {(
                [
                  ["skill", "Skill 文档"],
                  ["profile", "结构化资料"],
                  ["json", "JSON"],
                  ["import", "导入资料"],
                  ["edit", "编辑"],
                ] as [Tab, string][]
              ).map(([k, label]) => (
                <button
                  key={k}
                  type="button"
                  className={`persona-detail-tab${tab === k ? " active" : ""}`}
                  onClick={() => setTab(k)}
                >
                  {label}
                </button>
              ))}
            </div>

            <div className="persona-detail-body">
              {tab === "skill" && (
                <pre className="persona-md-preview">{detail.skill_md || "（暂无 Skill 文档）"}</pre>
              )}
              {tab === "profile" && <StructuredProfile data={detail.profile_json} />}
              {tab === "json" && <pre className="persona-json-preview">{jsonText}</pre>}
              {tab === "import" && (
                <PersonaTextImportForm
                  mode="update"
                  personaId={detail.id}
                  compact
                  onSuccess={onUpdated}
                />
              )}
              {tab === "edit" && <PersonaEditForm detail={detail} onSaved={onUpdated} />}
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}
