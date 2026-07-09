export type FeedbackTone = "loading" | "success" | "error" | "info";

export interface SettingsFeedback {
  tone: FeedbackTone;
  title: string;
  detail?: string;
  hint?: string;
  /** 成功态下的统计标签，如「2 文本」「0 多模态」 */
  tags?: string[];
}

/** 将 Tauri/后端抛出的原始错误转为可读反馈 */
export function parseApiError(raw: unknown, context = "操作"): SettingsFeedback {
  const text = normalizeErrorText(raw);
  const status = extractHttpStatus(text);
  const apiMessage = extractJsonMessage(text);
  const url = extractUrl(text);

  if (text.includes("请先配置 API 密钥") || text.includes("密码本未解锁")) {
    return {
      tone: "error",
      title: "未配置 API 密钥",
      detail: "请先在密码本保存密钥，并在上方供应商卡片中关联。",
      hint: "Ollama 本地服务无需密钥。",
    };
  }

  if (status === 401 || text.includes("AuthenticationError") || text.includes("Unauthorized")) {
    return {
      tone: "error",
      title: "密钥验证失败",
      detail: apiMessage ?? "API Key 无效、过期，或与当前供应商网关不匹配。",
      hint: pickAuthHint(text),
    };
  }

  if (status === 403 || text.includes("AccessDenied")) {
    return {
      tone: "error",
      title: "没有访问权限",
      detail: apiMessage ?? "当前密钥无权访问该接口或模型。",
      hint: "请在方舟控制台检查 API Key 权限与已开通的模型服务。",
    };
  }

  if (status === 404) {
    return {
      tone: "error",
      title: "接口地址不正确",
      detail: apiMessage ?? "请求的 API 路径不存在。",
      hint: url
        ? `请核对供应商 Base URL 是否正确。\n请求：${url}`
        : "Agent Plan 应使用 /api/plan/v3。",
    };
  }

  if (text.includes("模型列表接口返回非 JSON") || text.includes("解析模型列表失败")) {
    return {
      tone: "error",
      title: "无法解析模型列表",
      detail: stripNoise(apiMessage ?? text),
      hint: text.includes("opencode") || text.includes("api.opencode.ai")
        ? "OpenCode GO 正确 Base URL 为 https://opencode.ai/zen/go/v1（不是 api.opencode.ai）。请重启应用后重新测试。"
        : "请核对供应商 Base URL 是否正确，或使用「手动添加」填写模型 ID。",
    };
  }

  if (text.includes("不完整") || text.includes("被截断") || text.includes("EOF while parsing")) {
    return {
      tone: "error",
      title: "AI 返回的 JSON 不完整",
      detail: stripNoise(apiMessage ?? text),
      hint: "参考文本过长时模型输出可能被截断。请缩短文本、减少台词示例，或更换输出上限更高的思考模型。",
    };
  }

  if (
    text.includes("ilinkai.weixin.qq.com") ||
    text.includes("ilink/bot") ||
    text.includes("轮询二维码") ||
    text.includes("获取绑定二维码")
  ) {
    return {
      tone: "error",
      title: "无法连接微信 iLink 服务",
      detail: stripNoise(apiMessage ?? text),
      hint: "请确认本机可访问 https://ilinkai.weixin.qq.com，并检查系统代理/VPN。扫码后轮询约 1 分钟，网络波动会自动重试，请勿重复点击。",
    };
  }

  if (
    text.includes("连接失败") ||
    text.includes("Ping 连接失败") ||
    text.includes("API 请求失败") ||
    text.includes("error sending request") ||
    text.includes("无法连接 API") ||
    text.includes("network") ||
    text.includes("timed out") ||
    text.includes("请求超时")
  ) {
    return {
      tone: "error",
      title: "无法连接服务器",
      detail: stripNoise(apiMessage ?? text),
      hint: "请检查网络、代理设置，以及供应商服务是否可用。人设导入需等待 1～3 分钟，请勿重复点击。",
    };
  }

  if (text.includes("列表为空") || text.includes("未找到 data/models")) {
    return {
      tone: "error",
      title: "未获取到模型列表",
      detail: stripNoise(text),
      hint: "可尝试先「测试」供应商，或使用「手动添加」填写模型 ID。",
    };
  }

  if (text.includes("未知供应商")) {
    return {
      tone: "error",
      title: "供应商配置异常",
      detail: "当前选择的供应商在本地配置中不存在。",
      hint: "请重启应用或重新保存 AI 设置。",
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

function extractUrl(text: string): string | null {
  const m = text.match(/（(https?:\/\/[^）]+)）/);
  return m?.[1] ?? null;
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
    .replace(/\s*Agent Plan.*$/s, "")
    .replace(/\s*Coding Plan.*$/s, "")
    .trim();
}

function pickAuthHint(text: string): string {
  if (text.includes("/api/plan") || text.includes("Agent Plan")) {
    return "Agent Plan 需在方舟控制台「Agent Plan」页面创建专用 Key，Base URL 为 https://ark.cn-beijing.volces.com/api/plan/v3。";
  }
  if (text.includes("/api/coding") || text.includes("Coding Plan")) {
    return "Coding Plan 请使用 /api/coding/v3 网关及套餐专用 API Key。";
  }
  if (text.includes("opencode") || text.includes("api.opencode.ai")) {
    return "OpenCode GO 请在 opencode.ai 订阅 Go 套餐获取 API Key，Base URL 为 https://opencode.ai/zen/go/v1。";
  }
  return "请确认密钥与供应商 Base URL 属于同一套餐，且密钥未过期。";
}

/** 将测试接口返回的成功消息拆成标题、详情与标签 */
export function parseTestSuccess(
  message: string,
  importedText = 0,
  importedVision = 0,
): SettingsFeedback {
  let body = message
    .replace(/，已保存 \d+ 个文本 \/ \d+ 个多模态模型到本地$/, "")
    .replace(/^连接成功[，,]?\s*/, "")
    .trim();

  let detail: string | undefined;
  let hint: string | undefined;
  const paren = body.match(/^(.+?)[（(]([^）)]+)[）)]\s*$/);
  if (paren) {
    detail = paren[1].trim();
    hint = paren[2].trim();
  } else if (body) {
    detail = body;
  }

  const tags: string[] = [];
  if (importedText + importedVision > 0) {
    tags.push(`${importedText} 文本`);
    tags.push(`${importedVision} 多模态`);
  }

  return {
    tone: "success",
    title: "连接成功",
    detail,
    hint,
    tags: tags.length > 0 ? tags : undefined,
  };
}
