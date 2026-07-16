export type FeedbackTone = "loading" | "success" | "error" | "info";

export interface SettingsFeedback {
  tone: FeedbackTone;
  title: string;
  detail?: string;
  hint?: string;
  tags?: string[];
}

/** 将 Tauri/后端抛出的原始错误转为可读反馈（live2d-only） */
export function parseApiError(raw: unknown, context = "操作"): SettingsFeedback {
  const text = normalizeErrorText(raw);
  const status = extractHttpStatus(text);
  const apiMessage = extractJsonMessage(text);

  if (
    text.includes("BLHX_WIKI_DB_PATH") ||
    text.includes("未找到本地 BWIKI") ||
    text.includes("本地 BWIKI 库中未找到")
  ) {
    return {
      tone: "error",
      title: "本地 Wiki 库不可用",
      detail: stripNoise(apiMessage ?? text),
      hint: "请设置环境变量 BLHX_WIKI_DB_PATH 指向 blhx-wiki 的 SQLite，或改用在线 Wiki / 粘贴文本导入。",
    };
  }

  if (
    text.includes("无法从 Wiki 文本解析") ||
    text.includes("未能从页面中解析出台词") ||
    text.includes("参考文本不能为空") ||
    text.includes("参考文本为空")
  ) {
    return {
      tone: "error",
      title: "资料解析失败",
      detail: stripNoise(apiMessage ?? text),
      hint: "请确认 Wiki 页面包含角色简介/台词，或改用粘贴文本 / 本地库导入。",
    };
  }

  if (text.includes("未知人设") || text.includes("未知角色") || text.includes("未知人物")) {
    return {
      tone: "error",
      title: "人物不存在",
      detail: stripNoise(apiMessage ?? text),
      hint: "请刷新人物列表后重试。",
    };
  }

  if (text.includes("内置人设不可删除") || text.includes("跳过内置")) {
    return {
      tone: "error",
      title: "无法删除内置人物",
      detail: stripNoise(apiMessage ?? text),
    };
  }

  if (
    text.includes("桌宠窗口") ||
    (text.includes("模型") && text.includes("不存在")) ||
    text.includes("动作名不能为空")
  ) {
    return {
      tone: "error",
      title: "桌宠操作失败",
      detail: stripNoise(apiMessage ?? text),
      hint: "请确认当前皮肤已绑定可用模型，必要时重新导入 Live2D 文件。",
    };
  }

  if (text.includes("请输入 Wiki 链接") || text.includes("链接需以 http")) {
    return {
      tone: "error",
      title: "Wiki 链接无效",
      detail: stripNoise(apiMessage ?? text),
      hint: "请粘贴完整的 http(s) 链接，或只填写 BWIKI 舰娘名称。",
    };
  }

  if (text.includes("网页返回 HTTP") || text.includes("爬取失败") || text.includes("读取网页内容失败")) {
    return {
      tone: "error",
      title: "Wiki 页面获取失败",
      detail: stripNoise(apiMessage ?? text),
      hint: "请检查网络与链接是否可访问；BWIKI 有时需要稍等后重试。",
    };
  }

  if (
    text.includes("连接失败") ||
    text.includes("error sending request") ||
    text.includes("network") ||
    text.includes("timed out") ||
    text.includes("请求超时")
  ) {
    return {
      tone: "error",
      title: "网络请求失败",
      detail: stripNoise(apiMessage ?? text),
      hint: "Wiki 导入与头像下载需要联网。请检查代理/VPN，稍等后重试，勿重复点击。",
    };
  }

  if (status === 401 || status === 403) {
    return {
      tone: "error",
      title: "远程资源拒绝访问",
      detail: apiMessage ?? stripNoise(text),
      hint: "通常是目标 Wiki 或图床限制，请换链接或改用本地导入。",
    };
  }

  if (status === 404) {
    return {
      tone: "error",
      title: "页面不存在",
      detail: apiMessage ?? "请求的 Wiki 页面未找到。",
      hint: "请核对舰娘名称或链接是否正确。",
    };
  }

  if (text.includes("不完整") || text.includes("被截断") || text.includes("EOF while parsing")) {
    return {
      tone: "error",
      title: "资料格式无法解析",
      detail: stripNoise(apiMessage ?? text),
      hint: "粘贴文本时请保留完整段落；Wiki 页面结构异常时可改用手动编辑 JSON。",
    };
  }

  if (text.includes("纯桌宠分支不支持")) {
    return {
      tone: "error",
      title: "功能不可用",
      detail: stripNoise(text),
      hint: "当前为纯桌宠版本，该操作仅在完整版日报分支提供。",
    };
  }

  const detail = apiMessage ?? stripNoise(text);
  return {
    tone: "error",
    title: `${context}失败`,
    detail: detail.length > 280 ? `${detail.slice(0, 280)}…` : detail,
    hint: status ? `HTTP ${status}` : undefined,
  };
}

export function successFeedback(title: string, detail?: string): SettingsFeedback {
  return { tone: "success", title, detail };
}

export function loadingFeedback(title: string): SettingsFeedback {
  return { tone: "loading", title };
}

function normalizeErrorText(raw: unknown): string {
  let s = String(raw ?? "未知错误");
  s = s.replace(/^Error:\s*/i, "");
  s = s.replace(/^导入失败：/, "");
  s = s.replace(/^Invoke\s+/i, "");
  return s.trim();
}

function extractHttpStatus(text: string): number | null {
  const m = text.match(/HTTP\s+(\d{3})/i);
  return m ? parseInt(m[1], 10) : null;
}

function extractJsonMessage(text: string): string | null {
  const jsonStart = text.indexOf("{");
  if (jsonStart < 0) return null;
  try {
    const slice = text.slice(jsonStart);
    const end = slice.lastIndexOf("}");
    if (end < 0) return null;
    const obj = JSON.parse(slice.slice(0, end + 1)) as Record<string, unknown>;
    const err = obj.error as Record<string, unknown> | undefined;
    const msg =
      (err?.message as string | undefined) ??
      (obj.message as string | undefined) ??
      (obj.error_message as string | undefined);
    return msg?.trim() || null;
  } catch {
    const m = text.match(/"message"\s*:\s*"([^"]+)"/);
    return m?.[1]?.replace(/\\"/g, '"') ?? null;
  }
}

function stripNoise(text: string): string {
  return text
    .replace(/HTTP\s+\d{3}\s*·\s*/gi, "")
    .replace(/（https?:\/\/[^）]+）/g, "")
    .trim();
}
