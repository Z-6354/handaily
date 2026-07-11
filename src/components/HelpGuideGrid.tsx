import { useCallback, useEffect, useState } from "react";
import {
  cloneHelpContent,
  DEFAULT_HELP_CONTENT,
  HELP_CONTENT_SETTING_KEY,
  parseHelpContent,
  serializeHelpContent,
  type HelpContent,
} from "../lib/helpContent";
import { xiaohan } from "../lib/xiaohan";

const BILIBILI_HOME = "https://space.bilibili.com/146915875";
const IS_DEBUG = import.meta.env.DEV;

export function HelpGuideGrid() {
  const [content, setContent] = useState<HelpContent>(DEFAULT_HELP_CONTENT);
  const [draft, setDraft] = useState<HelpContent>(DEFAULT_HELP_CONTENT);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);

  const loadContent = useCallback(async () => {
    try {
      const raw = await xiaohan.getSetting(HELP_CONTENT_SETTING_KEY);
      const parsed = parseHelpContent(raw);
      setContent(parsed);
      setDraft(cloneHelpContent(parsed));
    } catch {
      setContent(DEFAULT_HELP_CONTENT);
      setDraft(cloneHelpContent(DEFAULT_HELP_CONTENT));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadContent();
  }, [loadContent]);

  const startEdit = () => {
    setDraft(cloneHelpContent(content));
    setEditing(true);
    setSaveMsg(null);
  };

  const cancelEdit = () => {
    setDraft(cloneHelpContent(content));
    setEditing(false);
    setSaveMsg(null);
  };

  const saveContent = async () => {
    setSaving(true);
    setSaveMsg(null);
    try {
      await xiaohan.saveSetting(HELP_CONTENT_SETTING_KEY, serializeHelpContent(draft));
      setContent(cloneHelpContent(draft));
      setEditing(false);
      setSaveMsg("已保存");
    } catch (e) {
      setSaveMsg(`保存失败：${String(e)}`);
    } finally {
      setSaving(false);
    }
  };

  const resetDefault = () => {
    setDraft(cloneHelpContent(DEFAULT_HELP_CONTENT));
    setSaveMsg(null);
  };

  const updateSection = (index: number, patch: Partial<HelpContent["sections"][0]>) => {
    setDraft((prev) => ({
      ...prev,
      sections: prev.sections.map((s, i) => (i === index ? { ...s, ...patch } : s)),
    }));
  };

  const updateChangelog = (index: number, patch: Partial<HelpContent["changelog"][0]>) => {
    setDraft((prev) => ({
      ...prev,
      changelog: prev.changelog.map((c, i) => (i === index ? { ...c, ...patch } : c)),
    }));
  };

  const view = editing ? draft : content;

  if (loading) {
    return (
      <div className="doc-shell">
        <p className="empty empty--compact">加载使用说明…</p>
      </div>
    );
  }

  return (
    <div className="doc-shell">
      <div className="doc-shell__grid">
        <nav className="doc-toc" aria-label="目录">
          <p className="doc-toc__label">目录</p>
          <a className="doc-toc__link" href="#doc-intro">
            简介
          </a>
          {view.sections.map((section) => (
            <a key={section.id} className="doc-toc__link" href={`#doc-${section.id}`}>
              {section.title}
            </a>
          ))}
          <a className="doc-toc__link" href="#doc-changelog">
            更新公告
          </a>
          <a className="doc-toc__link" href="#doc-feedback">
            反馈
          </a>
        </nav>

        <div className="doc-main">
          <section id="doc-intro" className="doc-lead">
            <div className="doc-lead__head">
              <h2 className="doc-lead__title">使用说明</h2>
              {IS_DEBUG && (
                <div className="doc-edit-bar">
                  {editing ? (
                    <>
                      <button
                        type="button"
                        className="btn-primary btn-sm"
                        disabled={saving}
                        onClick={() => void saveContent()}
                      >
                        {saving ? "保存中…" : "保存"}
                      </button>
                      <button
                        type="button"
                        className="btn-secondary btn-sm"
                        disabled={saving}
                        onClick={cancelEdit}
                      >
                        取消
                      </button>
                      <button
                        type="button"
                        className="btn-link btn-sm"
                        disabled={saving}
                        onClick={resetDefault}
                      >
                        恢复默认
                      </button>
                    </>
                  ) : (
                    <button type="button" className="btn-secondary btn-sm" onClick={startEdit}>
                      编辑
                    </button>
                  )}
                  {saveMsg && <span className="doc-edit-msg">{saveMsg}</span>}
                </div>
              )}
            </div>
            {editing ? (
              <textarea
                className="doc-field"
                value={draft.introLead}
                rows={3}
                onChange={(e) => setDraft((p) => ({ ...p, introLead: e.target.value }))}
              />
            ) : (
              <p className="doc-lead__text">{view.introLead}</p>
            )}
          </section>

          {view.sections.map((section, index) => (
            <article key={section.id} id={`doc-${section.id}`} className="doc-section">
              {editing ? (
                <input
                  className="doc-field doc-field--title"
                  value={draft.sections[index]?.title ?? section.title}
                  onChange={(e) => updateSection(index, { title: e.target.value })}
                />
              ) : (
                <h3 className="doc-section__title">{section.title}</h3>
              )}
              {editing ? (
                <textarea
                  className="doc-field"
                  value={draft.sections[index]?.body ?? section.body}
                  rows={5}
                  onChange={(e) => updateSection(index, { body: e.target.value })}
                />
              ) : (
                <p className="doc-section__body">{section.body}</p>
              )}
            </article>
          ))}

          <div className="doc-asides">
            <section id="doc-changelog" className="doc-aside">
              <h3 className="doc-aside__title">更新公告</h3>
              {view.changelog.map((entry, index) => (
                <article key={`${entry.version}-${index}`} className="doc-release">
                  <div className="doc-release__meta">
                    {editing ? (
                      <>
                        <input
                          className="doc-field doc-field--short doc-field--title"
                          value={draft.changelog[index]?.version ?? entry.version}
                          onChange={(e) => updateChangelog(index, { version: e.target.value })}
                          placeholder="版本"
                        />
                        <input
                          className="doc-field doc-field--short"
                          value={draft.changelog[index]?.date ?? entry.date}
                          onChange={(e) => updateChangelog(index, { date: e.target.value })}
                          placeholder="日期"
                        />
                      </>
                    ) : (
                      <>
                        <span className="doc-release__ver">v{entry.version}</span>
                        <time className="doc-release__date">{entry.date}</time>
                      </>
                    )}
                  </div>
                  {editing ? (
                    <textarea
                      className="doc-field"
                      value={draft.changelog[index]?.body ?? entry.body}
                      rows={3}
                      onChange={(e) => updateChangelog(index, { body: e.target.value })}
                    />
                  ) : (
                    <p className="doc-release__body">{entry.body}</p>
                  )}
                </article>
              ))}
            </section>

            <section id="doc-feedback" className="doc-aside">
              <h3 className="doc-aside__title">反馈</h3>
              {editing ? (
                <textarea
                  className="doc-field"
                  value={draft.footerText}
                  rows={3}
                  onChange={(e) => setDraft((p) => ({ ...p, footerText: e.target.value }))}
                />
              ) : view.footerText === DEFAULT_HELP_CONTENT.footerText ? (
                <p className="doc-footer-text">
                  使用中遇到问题，欢迎到 B 站主页
                  <a href={BILIBILI_HOME} target="_blank" rel="noopener noreferrer">
                    万年烟火
                  </a>
                  留言或私信。
                </p>
              ) : (
                <p className="doc-footer-text">{view.footerText}</p>
              )}
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}
