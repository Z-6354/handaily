import { useCallback, useEffect, useRef, useState } from "react";
import { SettingsFeedbackBanner } from "../components/SettingsFeedbackBanner";
import { SettingsToggle } from "../components/SettingsToggle";
import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan, type WechatStatus } from "../lib/xiaohan";

type BindPhase = "idle" | "polling" | "done";

const QR_STATUS: Record<string, string> = {
  wait: "等待扫码…",
  scaned: "已扫码，请在微信中确认",
  expired: "二维码已过期",
};

const MAX_POLL_FAILS = 8;

export function WeChatBindPage() {
  const [status, setStatus] = useState<WechatStatus | null>(null);
  const [phase, setPhase] = useState<BindPhase>("idle");
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [qrHint, setQrHint] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);
  const qrcodeIdRef = useRef<string | null>(null);
  const pollActiveRef = useRef(false);
  const pollFailRef = useRef(0);

  const stopPoll = useCallback(() => {
    pollActiveRef.current = false;
    qrcodeIdRef.current = null;
  }, []);

  const loadStatus = useCallback(async () => {
    try {
      const s = await xiaohan.wechatGetStatus();
      setStatus(s);
      if (s.bound) {
        setPhase("done");
        setQrDataUrl(null);
        setQrHint("");
        stopPoll();
      }
    } catch (e) {
      setFeedback(parseApiError(e, "读取微信绑定状态"));
    }
  }, [stopPoll]);

  useEffect(() => {
    void loadStatus();
    return () => stopPoll();
  }, [loadStatus, stopPoll]);

  useEffect(() => {
    if (!status?.bound || status.session_ready) return;
    const t = setInterval(() => void loadStatus(), 5000);
    return () => clearInterval(t);
  }, [status?.bound, status?.session_ready, loadStatus]);

  const runPollLoop = useCallback(async (qrcodeId: string) => {
    pollActiveRef.current = true;
    pollFailRef.current = 0;

    while (pollActiveRef.current && qrcodeIdRef.current === qrcodeId) {
      try {
        const res = await xiaohan.wechatPollQr(qrcodeId);
        pollFailRef.current = 0;
        setFeedback(null);

        if (res.status === "confirmed" && res.bound) {
          stopPoll();
          setPhase("done");
          setQrDataUrl(null);
          setQrHint("");
          setFeedback(successFeedback("微信绑定成功"));
          await loadStatus();
          return;
        }
        if (res.status === "expired") {
          stopPoll();
          setPhase("idle");
          setQrDataUrl(null);
          setQrHint("");
          setFeedback(parseApiError(new Error(res.retmsg ?? "二维码已过期"), "绑定"));
          return;
        }
        setQrHint(QR_STATUS[res.status] ?? "等待确认…");
      } catch (e) {
        pollFailRef.current += 1;
        if (pollFailRef.current >= MAX_POLL_FAILS) {
          stopPoll();
          setPhase("idle");
          setFeedback(parseApiError(e, "扫码确认"));
          return;
        }
        setQrHint(`连接波动，继续等待扫码… (${pollFailRef.current}/${MAX_POLL_FAILS})`);
      }

      if (!pollActiveRef.current) return;
      await new Promise((r) => setTimeout(r, 800));
    }
  }, [loadStatus, stopPoll]);

  const startBind = async () => {
    setBusy(true);
    setFeedback(null);
    stopPoll();
    setQrHint("");
    try {
      const qr = await xiaohan.wechatStartQr();
      qrcodeIdRef.current = qr.qrcode_id;
      setQrDataUrl(qr.qrcode_data_url);
      setPhase("polling");
      setQrHint(QR_STATUS.wait);
      void runPollLoop(qr.qrcode_id);
    } catch (e) {
      setPhase("idle");
      setFeedback(parseApiError(e, "获取绑定二维码"));
    } finally {
      setBusy(false);
    }
  };

  const logout = async () => {
    setBusy(true);
    setFeedback(null);
    stopPoll();
    try {
      await xiaohan.wechatLogout();
      setPhase("idle");
      setQrDataUrl(null);
      setFeedback(successFeedback("已解绑微信"));
      await loadStatus();
    } catch (e) {
      setFeedback(parseApiError(e, "解绑微信"));
    } finally {
      setBusy(false);
    }
  };

  const togglePush = async (enabled: boolean) => {
    setBusy(true);
    setFeedback(null);
    try {
      await xiaohan.wechatSetPushEnabled(enabled);
      setFeedback(successFeedback(enabled ? "微信推送已开启" : "微信推送已关闭"));
      await loadStatus();
    } catch (e) {
      setFeedback(parseApiError(e, "切换推送"));
    } finally {
      setBusy(false);
    }
  };

  const testSend = async () => {
    setBusy(true);
    setFeedback(null);
    try {
      const msg = await xiaohan.wechatTestSend();
      setFeedback(successFeedback(msg));
      await loadStatus();
    } catch (e) {
      setFeedback(parseApiError(e, "测试发送"));
    } finally {
      setBusy(false);
    }
  };

  const rebind = async () => {
    setBusy(true);
    setFeedback(null);
    stopPoll();
    try {
      await xiaohan.wechatPrepareRebind();
      setPhase("idle");
      setQrDataUrl(null);
      setQrHint("");
      await loadStatus();
      setFeedback(successFeedback("已清除旧绑定，请扫码重新绑定"));
      setBusy(false);
      await startBind();
    } catch (e) {
      setFeedback(parseApiError(e, "重新绑定"));
      setBusy(false);
    }
  };

  const bound = status?.bound ?? false;
  const pushEnabled = status?.push_enabled ?? false;
  const needsRebind = status?.needs_rebind ?? false;
  const binding = phase === "polling";
  const showQrPanel = !bound || binding || !!qrDataUrl;

  return (
    <div className="page-stack wechat-page">
      <SettingsFeedbackBanner feedback={feedback} />

      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">绑定状态</div>
        </div>
        <div className="settings-field-stack">
          <p className="settings-field-hint">
            使用微信 iLink ClawBot 插件扫码绑定，凭证保存在小寒日报本地（独立 Agent，不依赖外部工具）。
          </p>
          <dl className="wechat-status-dl">
            <div>
              <dt>状态</dt>
              <dd>{bound ? "已绑定" : "未绑定"}</dd>
            </div>
            {bound && status?.account_id && (
              <div>
                <dt>Bot ID</dt>
                <dd><code>{status.account_id}</code></dd>
              </div>
            )}
            <div>
              <dt>推送通道</dt>
              <dd>{status?.session_ready ? "已激活" : "待激活"}</dd>
            </div>
            {(status?.pending_count ?? 0) > 0 && (
              <div>
                <dt>待发消息</dt>
                <dd>{status?.pending_count} 条</dd>
              </div>
            )}
          </dl>
          {status?.hint && <p className="hint-block">{status.hint}</p>}
        </div>
      </div>

      {needsRebind && bound && !showQrPanel && (
        <div className="panel wechat-rebind-panel">
          <div className="panel-header">
            <div className="panel-title">需要重新绑定</div>
          </div>
          <div className="settings-field-stack">
            <p className="hint-block">
              当前 Bot 无法激活推送通道。请重新扫码，让小寒日报获得独立的 ClawBot 凭证。
            </p>
            <div className="wechat-actions">
              <button type="button" className="btn-primary" disabled={busy} onClick={() => void rebind()}>
                重新扫码绑定
              </button>
              <button type="button" className="btn-link wechat-logout" disabled={busy} onClick={() => void logout()}>
                仅解绑
              </button>
            </div>
          </div>
        </div>
      )}

      {showQrPanel && (
        <div className="panel">
          <div className="panel-header">
            <div className="panel-title">扫码绑定</div>
          </div>
          <div className="wechat-qr-block">
            {qrDataUrl ? (
              <>
                <img className="wechat-qr-img" src={qrDataUrl} alt="微信绑定二维码" width={320} height={320} />
                <p className="hint-block">{qrHint || "请用微信扫描二维码"}</p>
                {binding && (
                  <p className="hint-block">
                    扫码后轮询可能持续约 1 分钟，请耐心等待，勿重复点击。
                  </p>
                )}
              </>
            ) : (
              <p className="hint-block">
                {needsRebind
                  ? "正在准备重新绑定，请用微信扫描二维码"
                  : "点击下方按钮获取二维码，用微信 ClawBot 扫码确认"}
              </p>
            )}
            <div className="wechat-actions">
              <button type="button" className="btn-primary" disabled={busy || binding} onClick={() => void startBind()}>
                {binding ? "等待扫码确认…" : qrDataUrl ? "刷新二维码" : "开始绑定"}
              </button>
            </div>
          </div>
        </div>
      )}

      {bound && status?.session_ready && (
        <div className="panel">
          <div className="panel-header">
            <div className="panel-title">推送设置</div>
          </div>
          <div className="settings-field-stack">
            <SettingsToggle
              label="启用微信推送"
              hint="启动通知、整点小结、昨日日报"
              checked={pushEnabled}
              disabled={busy}
              onChange={(v) => void togglePush(v)}
            />
            <ul className="hint-block wechat-push-list">
              <li>每次启动应用后发送启动消息</li>
              <li>每小时整点后 10 分钟内发送上一小时活动小结</li>
              <li>每天凌晨发送昨日日报</li>
            </ul>
            <div className="wechat-actions">
              <button type="button" className="btn-secondary" disabled={busy} onClick={() => void testSend()}>
                测试发送
              </button>
              <button type="button" className="btn-link wechat-logout" disabled={busy} onClick={() => void logout()}>
                解绑微信
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
