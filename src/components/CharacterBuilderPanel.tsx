import { useCallback, useEffect, useState } from "react";
import { SettingsFeedbackBanner } from "../components/SettingsFeedbackBanner";
import { SettingsSection } from "../components/SettingsSection";
import {
  loadingFeedback,
  parseApiError,
  successFeedback,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import {
  xiaohan,
  type CharacterProfile,
  type CharacterProfileData,
} from "../lib/xiaohan";

const EMPTY_PROFILE: CharacterProfileData = {
  name: "",
  source: "",
  introduction: "",
  personality: [],
  speech_style: "",
  sample_lines: [],
  relationships: "",
  taboos: [],
  extra: {},
};

type Props = {
  onPersonaApplied?: () => void;
};

export function CharacterBuilderPanel({ onPersonaApplied }: Props) {
  const [profiles, setProfiles] = useState<CharacterProfile[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [profile, setProfile] = useState<CharacterProfile | null>(null);
  const [jsonText, setJsonText] = useState("");
  const [skillMd, setSkillMd] = useState("");
  const [rawText, setRawText] = useState("");
  const [mergeText, setMergeText] = useState("");
  const [newName, setNewName] = useState("");
  const [newSource, setNewSource] = useState("");
  const [newRaw, setNewRaw] = useState("");
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);
  const [busy, setBusy] = useState(false);

  const loadList = useCallback(async () => {
    const list = await xiaohan.characterList();
    setProfiles(list);
    return list;
  }, []);

  const selectProfile = useCallback(async (id: number) => {
    const p = await xiaohan.characterGet(id);
    setSelectedId(id);
    setProfile(p);
    setRawText(p.raw_text);
    setSkillMd(p.skill_md);
    setJsonText(JSON.stringify(p.profile_json, null, 2));
  }, []);

  useEffect(() => {
    loadList().catch(() => setProfiles([]));
  }, [loadList]);

  const applyProfile = (p: CharacterProfile, message: string) => {
    setProfile(p);
    setRawText(p.raw_text);
    setSkillMd(p.skill_md);
    setJsonText(JSON.stringify(p.profile_json, null, 2));
    setFeedback(successFeedback(message));
    loadList();
  };

  const run = async (label: string, fn: () => Promise<{ profile: CharacterProfile; message: string }>) => {
    setBusy(true);
    setFeedback(loadingFeedback(label));
    try {
      const result = await fn();
      applyProfile(result.profile, result.message);
    } catch (e) {
      setFeedback(parseApiError(e, label));
    } finally {
      setBusy(false);
    }
  };

  const createProfile = async () => {
    if (!newName.trim()) {
      setFeedback(parseApiError("请填写角色名", "创建"));
      return;
    }
    setBusy(true);
    setFeedback(loadingFeedback("正在创建角色…"));
    try {
      const p = await xiaohan.characterCreate(newName.trim(), newSource.trim(), newRaw);
      setNewName("");
      setNewSource("");
      setNewRaw("");
      await loadList();
      await selectProfile(p.id);
      setFeedback(successFeedback("角色已创建", "可继续解析文本或编辑资料。"));
    } catch (e) {
      setFeedback(parseApiError(e, "创建"));
    } finally {
      setBusy(false);
    }
  };

  const saveJson = async () => {
    if (!selectedId) return;
    setBusy(true);
    setFeedback(loadingFeedback("正在保存 JSON…"));
    try {
      const data = JSON.parse(jsonText) as CharacterProfileData;
      const p = await xiaohan.characterUpdateJson(selectedId, data);
      applyProfile(p, "结构化资料已保存");
    } catch (e) {
      setFeedback(parseApiError(e, "保存 JSON"));
    } finally {
      setBusy(false);
    }
  };

  const saveRaw = async () => {
    if (!selectedId) return;
    setBusy(true);
    setFeedback(loadingFeedback("正在保存原始文本…"));
    try {
      const p = await xiaohan.characterUpdateRaw(selectedId, rawText);
      applyProfile(p, "原始文本已保存");
    } catch (e) {
      setFeedback(parseApiError(e, "保存文本"));
    } finally {
      setBusy(false);
    }
  };

  const saveSkill = async () => {
    if (!selectedId) return;
    setBusy(true);
    setFeedback(loadingFeedback("正在保存 Skill…"));
    try {
      const p = await xiaohan.characterSaveSkill(selectedId, skillMd);
      applyProfile(p, "Skill 文档已保存");
    } catch (e) {
      setFeedback(parseApiError(e, "保存 Skill"));
    } finally {
      setBusy(false);
    }
  };

  const deleteCurrent = async () => {
    if (!selectedId || !profile) return;
    if (!window.confirm(`确定删除「${profile.name}」的资料？`)) return;
    setBusy(true);
    try {
      await xiaohan.characterDelete(selectedId);
      setSelectedId(null);
      setProfile(null);
      await loadList();
      setFeedback(successFeedback("已删除"));
    } catch (e) {
      setFeedback(parseApiError(e, "删除"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="panel settings-card character-builder">
      <SettingsSection
        title="人设工坊"
        description="导入角色复合文本 → AI 解析为结构化资料 → 生成 Skill 人设文档 → 写入 AI 人设"
      >
        <div className="character-builder-layout">
          <aside className="character-builder-list">
            <div className="character-builder-new">
              <input
                className="settings-input"
                placeholder="角色名"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
              />
              <input
                className="settings-input"
                placeholder="出处（可选）"
                value={newSource}
                onChange={(e) => setNewSource(e.target.value)}
              />
              <textarea
                className="settings-textarea"
                rows={4}
                placeholder="粘贴角色介绍、设定、台词等大段文本…"
                value={newRaw}
                onChange={(e) => setNewRaw(e.target.value)}
              />
              <button type="button" className="btn-secondary btn-sm" disabled={busy} onClick={createProfile}>
                新建角色
              </button>
            </div>
            <div className="character-builder-items">
              {profiles.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  className={`character-builder-item${selectedId === p.id ? " active" : ""}`}
                  onClick={() => selectProfile(p.id)}
                >
                  <div className="character-builder-item-name">{p.name}</div>
                  {p.source && <div className="character-builder-item-source">{p.source}</div>}
                </button>
              ))}
              {profiles.length === 0 && (
                <p className="settings-field-hint">还没有角色资料，先新建一个吧</p>
              )}
            </div>
          </aside>

          {profile ? (
            <div className="character-builder-editor">
              <div className="character-builder-steps">
                <div className="character-builder-step">
                  <h4>① 原始文本</h4>
                  <textarea
                    className="settings-textarea"
                    rows={6}
                    value={rawText}
                    onChange={(e) => setRawText(e.target.value)}
                  />
                  <div className="model-pick-actions">
                    <button type="button" className="btn-secondary btn-sm" disabled={busy} onClick={saveRaw}>
                      保存文本
                    </button>
                    <button
                      type="button"
                      className="btn-primary btn-sm"
                      disabled={busy}
                      onClick={() => run("AI 正在解析文本…", () => xiaohan.characterPreprocess(profile.id))}
                    >
                      AI 解析为结构化资料
                    </button>
                  </div>
                </div>

                <div className="character-builder-step">
                  <h4>② 结构化资料（JSON）</h4>
                  <textarea
                    className="settings-textarea character-json-editor"
                    rows={12}
                    value={jsonText}
                    onChange={(e) => setJsonText(e.target.value)}
                    spellCheck={false}
                  />
                  <div className="model-pick-actions">
                    <button type="button" className="btn-secondary btn-sm" disabled={busy} onClick={saveJson}>
                      保存 JSON
                    </button>
                  </div>
                  <textarea
                    className="settings-textarea"
                    rows={3}
                    placeholder="补充文本（合并进已有 JSON）…"
                    value={mergeText}
                    onChange={(e) => setMergeText(e.target.value)}
                  />
                  <button
                    type="button"
                    className="btn-secondary btn-sm"
                    disabled={busy || !mergeText.trim()}
                    onClick={() =>
                      run("AI 正在合并文本…", async () => {
                        const r = await xiaohan.characterMergeText(profile.id, mergeText);
                        setMergeText("");
                        return r;
                      })
                    }
                  >
                    用文本更新资料
                  </button>
                </div>

                <div className="character-builder-step">
                  <h4>③ Skill 人设文档</h4>
                  <textarea
                    className="settings-textarea"
                    rows={12}
                    value={skillMd}
                    onChange={(e) => setSkillMd(e.target.value)}
                  />
                  <div className="model-pick-actions">
                    <button
                      type="button"
                      className="btn-primary btn-sm"
                      disabled={busy}
                      onClick={() => run("AI 正在生成 Skill…", () => xiaohan.characterGenerateSkill(profile.id))}
                    >
                      生成 / 更新 Skill
                    </button>
                    <button type="button" className="btn-secondary btn-sm" disabled={busy} onClick={saveSkill}>
                      保存编辑
                    </button>
                    <button
                      type="button"
                      className="btn-secondary btn-sm"
                      disabled={busy}
                      onClick={() =>
                        run("正在写入人设…", async () => {
                          const r = await xiaohan.characterApplyPersona(profile.id, true);
                          onPersonaApplied?.();
                          return r;
                        })
                      }
                    >
                      应用到 AI 人设
                    </button>
                    <button type="button" className="btn-link danger" disabled={busy} onClick={deleteCurrent}>
                      删除
                    </button>
                  </div>
                  {profile.persona_id && (
                    <p className="settings-field-hint">
                      已关联人设 ID：<code>{profile.persona_id}</code>
                    </p>
                  )}
                </div>
              </div>
            </div>
          ) : (
            <div className="character-builder-empty">
              <p className="hint-block">选择左侧角色，或新建后按步骤操作。</p>
              <p className="settings-field-hint">
                解析与 Skill 生成使用设置中的<strong>思考模型</strong>（未配置时回退到文本模型）。
              </p>
            </div>
          )}
        </div>
        <SettingsFeedbackBanner feedback={feedback} />
      </SettingsSection>
    </div>
  );
}

export { EMPTY_PROFILE };
