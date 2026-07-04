import { useEffect, useState } from "react";
import { SettingsFeedbackBanner } from "./SettingsFeedbackBanner";
import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan, type CharacterProfileData, type PersonaDetail } from "../lib/xiaohan";

type Props = {
  detail: PersonaDetail;
  onSaved: () => void | Promise<void>;
};

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

export function PersonaEditForm({ detail, onSaved }: Props) {
  const [name, setName] = useState(detail.name);
  const [source, setSource] = useState(detail.source);
  const [description, setDescription] = useState(detail.description);
  const [skillMd, setSkillMd] = useState(detail.skill_md);
  const [profileJson, setProfileJson] = useState(
    () => JSON.stringify(detail.profile_json, null, 2),
  );
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);

  useEffect(() => {
    setName(detail.name);
    setSource(detail.source);
    setDescription(detail.description);
    setSkillMd(detail.skill_md);
    setProfileJson(JSON.stringify(detail.profile_json, null, 2));
    setFeedback(null);
  }, [detail.id, detail.name, detail.source, detail.description, detail.skill_md, detail.profile_json]);

  const save = async () => {
    setSaving(true);
    setFeedback(null);
    try {
      let profile: CharacterProfileData;
      try {
        profile = JSON.parse(profileJson) as CharacterProfileData;
      } catch {
        throw new Error("结构化资料 JSON 格式不正确");
      }
      await xiaohan.personaUpdate(detail.id, {
        name: name.trim(),
        source: source.trim(),
        description: description.trim(),
        skill_md: skillMd,
        profile_json: profile,
      });
      setFeedback(successFeedback("已保存修改"));
      await onSaved();
    } catch (e) {
      setFeedback(parseApiError(e, "保存人设"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="persona-edit-form">
      <div className="persona-edit-section">
        <div className="persona-edit-section-title">基本信息</div>
        <div className="persona-edit-fields">
          <label className="persona-edit-label">
            显示名称
            <input
              className="persona-edit-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label className="persona-edit-label">
            来源
            <input
              className="persona-edit-input"
              value={source}
              onChange={(e) => setSource(e.target.value)}
              placeholder="可选"
            />
          </label>
          <label className="persona-edit-label persona-edit-label--full">
            简介
            <input
              className="persona-edit-input"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="卡片上显示的简短说明"
            />
          </label>
        </div>
      </div>

      <div className="persona-edit-section">
        <div className="persona-edit-section-title">Skill 文档</div>
        <textarea
          className="persona-edit-textarea"
          rows={10}
          value={skillMd}
          onChange={(e) => setSkillMd(e.target.value)}
          placeholder="Markdown 人设文档"
        />
      </div>

      <div className="persona-edit-section">
        <div className="persona-edit-section-title">结构化资料（JSON）</div>
        <textarea
          className="persona-edit-textarea persona-edit-textarea--mono"
          rows={12}
          value={profileJson}
          onChange={(e) => setProfileJson(e.target.value)}
          spellCheck={false}
        />
      </div>

      <div className="persona-edit-actions">
        <button type="button" className="btn-primary btn-sm" disabled={saving} onClick={save}>
          {saving ? "保存中…" : "保存修改"}
        </button>
        <button
          type="button"
          className="btn-secondary btn-sm"
          disabled={saving}
          onClick={() => {
            setName(detail.name);
            setSource(detail.source);
            setDescription(detail.description);
            setSkillMd(detail.skill_md);
            setProfileJson(JSON.stringify(detail.profile_json ?? EMPTY_PROFILE, null, 2));
            setFeedback(null);
          }}
        >
          还原
        </button>
      </div>

      <SettingsFeedbackBanner feedback={feedback} compact />
    </div>
  );
}
