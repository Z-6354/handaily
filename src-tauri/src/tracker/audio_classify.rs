//! 音频活动分类：听歌 / 看视频 / 聊天

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioActivity {
    Music,
    Video,
    Chat,
    Other,
}

impl AudioActivity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Video => "video",
            Self::Chat => "chat",
            Self::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Music => "听歌",
            Self::Video => "看视频",
            Self::Chat => "聊天通话",
            Self::Other => "音频",
        }
    }
}

/// 根据进程与音频会话显示名推断活动类型
pub fn classify_audio(exe_path: &str, app_name: &str, display_name: &str) -> AudioActivity {
    let exe = exe_path.to_lowercase();
    let app = app_name.to_lowercase();
    let disp = display_name.to_lowercase();
    let hay = format!("{exe} {app} {disp}");

    if hay.contains("spotify")
        || hay.contains("cloudmusic")
        || hay.contains("netease")
        || hay.contains("qqmusic")
        || hay.contains("kugou")
        || hay.contains("kuwo")
        || hay.contains("foobar")
        || hay.contains("aimp")
        || hay.contains("music")
    {
        return AudioActivity::Music;
    }

    if hay.contains("vlc")
        || hay.contains("potplayer")
        || hay.contains("mpc-hc")
        || hay.contains("bilibili")
        || hay.contains("youtube")
        || hay.contains("iqiyi")
        || hay.contains("youku")
        || hay.contains("qqvideo")
        || hay.contains("douyin")
        || hay.contains("tiktok")
        || hay.contains("播放")
        || hay.contains("video")
    {
        return AudioActivity::Video;
    }

    if hay.contains("discord")
        || hay.contains("teams")
        || hay.contains("zoom")
        || hay.contains("feishu")
        || hay.contains("lark")
        || hay.contains("wechat")
        || hay.contains("wxwork")
        || hay.contains("telegram")
        || hay.contains("skype")
        || hay.contains("voice")
        || hay.contains("通话")
        || hay.contains("语音")
        || hay.contains("会议")
    {
        return AudioActivity::Chat;
    }

    AudioActivity::Other
}

/// 无持续音频意义的后台进程（即便偶发系统音也不单独记时间线）
pub fn is_passive_background_process(exe_path: &str, app_name: &str) -> bool {
    let exe = exe_path.to_lowercase();
    let app = app_name.to_lowercase();
    let passive = [
        "systemsettings",
        "settings",
        "applicationframehost",
        "shellexperiencehost",
        "searchhost",
        "startmenuexperiencehost",
        "textinputhost",
        "explorer.exe",
        "dwm.exe",
        "audiodg.exe",
    ];
    if passive.iter().any(|p| exe.contains(p)) {
        return true;
    }
    // 微信/QQ 等仅挂后台、无通话时不通过音频检测写入（需有实际音频峰值且持续）
    if app == "wechat" || app == "weixin" || app == "qq" {
        return false; // 有音频时可能是通话，交给峰值+时长过滤
    }
    app == "settings" || app.contains("设置")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_music() {
        assert_eq!(
            classify_audio(r"C:\Apps\Spotify.exe", "Spotify", "Song"),
            AudioActivity::Music
        );
    }

    #[test]
    fn classify_chat() {
        assert_eq!(
            classify_audio(r"C:\Apps\Discord.exe", "Discord", "Voice"),
            AudioActivity::Chat
        );
    }

    #[test]
    fn passive_settings() {
        assert!(is_passive_background_process(
            r"C:\Windows\ImmersiveControlPanel\SystemSettings.exe",
            "SystemSettings"
        ));
    }
}
