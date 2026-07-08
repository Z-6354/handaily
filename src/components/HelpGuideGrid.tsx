const GUIDE_ITEMS = [
  { emoji: "📊", title: "今日工作", desc: "一眼看懂今天的节奏" },
  { emoji: "✨", title: "生成报告", desc: "把碎片收成一篇小记" },
  { emoji: "🕐", title: "工作时间线", desc: "按时间翻应用切换" },
  { emoji: "💬", title: "AI 人设", desc: "换语气，更像在聊天" },
  { emoji: "🐾", title: "人物", desc: "性格、皮肤与桌宠一体管理" },
  { emoji: "🔐", title: "密码本", desc: "密钥本地加密保存" },
] as const;

const BILIBILI_HOME = "https://space.bilibili.com/146915875";

export function HelpGuideGrid() {
  return (
    <div className="help-guide">
      <div className="help-guide-hero">
        <span className="help-guide-mascot" aria-hidden>
          ❄️
        </span>
        <p className="help-guide-lead">
          小寒在后台悄悄记下你用了什么、待了多久，数据只存在本机。
        </p>
      </div>
      <div className="help-guide-grid">
        {GUIDE_ITEMS.map((item) => (
          <div key={item.title} className="help-guide-card">
            <span className="help-guide-emoji" aria-hidden>
              {item.emoji}
            </span>
            <div className="help-guide-card-body">
              <div className="help-guide-card-title">{item.title}</div>
              <div className="help-guide-card-desc">{item.desc}</div>
            </div>
          </div>
        ))}
      </div>
      <div className="help-guide-contact">
        <div className="help-guide-contact-title">反馈与联系</div>
        <p className="help-guide-contact-desc">
          使用中遇到问题，欢迎到我的 B 站主页
          <a
            className="help-guide-link"
            href={BILIBILI_HOME}
            target="_blank"
            rel="noopener noreferrer"
          >
            万年烟火
          </a>
          反馈：可在视频下方评论，或直接私信我。
        </p>
      </div>
    </div>
  );
}
