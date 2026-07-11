export const HELP_CONTENT_SETTING_KEY = "help_custom_content_v1";
export const UPDATE_ANNOUNCEMENT_SEEN_KEY = "update_announcement_seen_v0.1.0";
export const CREATOR_BILIBILI_URL = "https://space.bilibili.com/146915875";

export interface HelpSection {
  id: string;
  icon: string;
  title: string;
  body: string;
}

export interface HelpChangelogEntry {
  version: string;
  date: string;
  body: string;
}

export interface HelpContent {
  introLead: string;
  sections: HelpSection[];
  changelog: HelpChangelogEntry[];
  footerText: string;
}

export const DEFAULT_HELP_CONTENT: HelpContent = {
  introLead:
    "小寒桌宠是碧蓝航线主题的 Live2D 桌宠启动器。人物、台词与动作都在本地管理，不依赖联网 AI，数据只保存在你的电脑上。",
  sections: [
    {
      id: "persona",
      icon: "🐾",
      title: "人物与模型",
      body: "打开「人物」页浏览角色列表，点进详情可切换皮肤与 Live2D 模型。收藏常用角色后，可在列表「收藏」筛选中快速找到；桌宠右键「切换人物」也只显示已收藏角色，若尚未收藏任何角色会提示「当前没有收藏角色」。桌宠右键「切换模型」列出当前选用角色的全部模型（含当前使用项），样式与切换人物相同。所有模型、头像与台词资料都保存在本机数据目录。",
    },
    {
      id: "lines",
      icon: "💬",
      title: "台词管理",
      body: "在人物详情里的「台词」标签查看和编辑台词，可为每条台词绑定特定动作；未绑定的台词会在任意动作触发时随机出现。单条导入可在「台词导入」粘贴文本或 Wiki 链接，批量导入请前往设置页。",
    },
    {
      id: "actions",
      icon: "🎭",
      title: "动作配置",
      body: "「动作」页为待机、点击、开机、回待机、拖拽与随机动作指定动画。点击动作名可预览效果，待机类动作会以循环方式播放。随机动作频率需在「动作分配」中勾选随机动作后才会生效。修改后会自动保存到当前模型。",
    },
    {
      id: "display",
      icon: "✨",
      title: "桌宠显示",
      body: "设置页可开关桌宠与台词气泡。关闭桌宠会销毁桌宠窗口，再次开启或重启应用可恢复。台词气泡控制点击、随机与定时台词的显示；气泡频率从台词库随机抽取定时台词，需先开启台词气泡。\n\n角色大小作用于所有模型，也可在桌宠右键「编辑范围」中用滚轮缩放。空闲阈值（30～600 秒）用于全屏或长时间无操作后自动隐藏桌宠。随机动作频率在设置页调节，需先在人物详情的动作页勾选随机动作。\n\n开机自启动仅托盘常驻，不会自动弹出主窗口。",
    },
    {
      id: "wiki-bulk",
      icon: "📥",
      title: "批量 Wiki 导入",
      body: "在设置 →「台词导入」点击「全部导入 Wiki 台词」。系统按 BWIKI 角色名（如「柴郡」，不含皮肤后缀）爬取台词，并写入该角色全部皮肤。已导入的模型会跳过，无法爬取的角色计入失败；进度在弹窗中显示，可随时暂停或停止。每批 100 个角色、串行请求防限流，不会在启动时自动执行。",
    },
    {
      id: "data",
      icon: "🔒",
      title: "数据与隐私",
      body: "人物、模型与台词均保存在本地，无需云端 AI。设置页底部可查看数据目录路径；如需备份或迁移，直接复制整个数据目录即可。",
    },
  ],
  changelog: [
    {
      version: "0.1.0",
      date: "2026-07",
      body: "批量 Wiki 台词改为按角色聚合导入；进度改为弹窗并支持暂停与停止；已导入模型不再重复写入，失败角色单独统计；帮助页改为段落说明。",
    },
  ],
  footerText: "使用中遇到问题，欢迎到 B 站主页留言或私信。",
};

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}

function pickString(obj: Record<string, unknown>, key: string, fallback: string): string {
  const v = obj[key];
  return typeof v === "string" ? v : fallback;
}

export function parseHelpContent(raw: string | null | undefined): HelpContent {
  if (!raw?.trim()) return DEFAULT_HELP_CONTENT;
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed)) return DEFAULT_HELP_CONTENT;

    const sectionsRaw = Array.isArray(parsed.sections) ? parsed.sections : [];
    const changelogRaw = Array.isArray(parsed.changelog) ? parsed.changelog : [];

    const sections: HelpSection[] = sectionsRaw
      .filter(isRecord)
      .map((s, i) => ({
        id: pickString(s, "id", `section-${i}`),
        icon: pickString(s, "icon", "📄"),
        title: pickString(s, "title", "未命名"),
        body: pickString(s, "body", ""),
      }))
      .filter((s) => s.title.trim() || s.body.trim());

    const changelog: HelpChangelogEntry[] = changelogRaw
      .filter(isRecord)
      .map((c) => ({
        version: pickString(c, "version", "0.0.0"),
        date: pickString(c, "date", ""),
        body: pickString(c, "body", ""),
      }))
      .filter((c) => c.version.trim() || c.body.trim());

    return {
      introLead: pickString(parsed, "introLead", DEFAULT_HELP_CONTENT.introLead),
      sections: sections.length > 0 ? sections : DEFAULT_HELP_CONTENT.sections,
      changelog: changelog.length > 0 ? changelog : DEFAULT_HELP_CONTENT.changelog,
      footerText: pickString(parsed, "footerText", DEFAULT_HELP_CONTENT.footerText),
    };
  } catch {
    return DEFAULT_HELP_CONTENT;
  }
}

export function serializeHelpContent(content: HelpContent): string {
  return JSON.stringify(content, null, 2);
}

export function cloneHelpContent(content: HelpContent): HelpContent {
  return JSON.parse(serializeHelpContent(content)) as HelpContent;
}
