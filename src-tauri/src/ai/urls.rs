//! OpenAI 兼容 API 路径拼接（火山 / DeepSeek / GLM 等 base_url 形态不同）

/// `POST …/chat/completions`
pub fn chat_completions_url(base_url: &str) -> String {
    join_api_path(base_url, "chat/completions")
}

/// `GET …/models`
pub fn models_list_url(base_url: &str) -> String {
    join_api_path(base_url, "models")
}

fn join_api_path(base_url: &str, suffix: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with(suffix) {
        return base.to_string();
    }
    // 已含版本段：…/v1、…/v3、…/v4、…/paas/v4
    if has_api_version_suffix(base) {
        return format!("{base}/{suffix}");
    }
    format!("{base}/v1/{suffix}")
}

fn has_api_version_suffix(base: &str) -> bool {
    base.ends_with("/v1")
        || base.ends_with("/v2")
        || base.ends_with("/v3")
        || base.ends_with("/v4")
        || base.contains("/v1/")
        || base.contains("/v3/")
        || base.contains("/v4/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_plan_urls() {
        let base = "https://ark.cn-beijing.volces.com/api/plan/v3";
        assert_eq!(
            models_list_url(base),
            "https://ark.cn-beijing.volces.com/api/plan/v3/models"
        );
        assert_eq!(
            chat_completions_url(base),
            "https://ark.cn-beijing.volces.com/api/plan/v3/chat/completions"
        );
    }

    #[test]
    fn volcano_urls() {
        let base = "https://ark.cn-beijing.volces.com/api/v3";
        assert_eq!(
            models_list_url(base),
            "https://ark.cn-beijing.volces.com/api/v3/models"
        );
        assert_eq!(
            chat_completions_url(base),
            "https://ark.cn-beijing.volces.com/api/v3/chat/completions"
        );
    }

    #[test]
    fn deepseek_urls() {
        let base = "https://api.deepseek.com";
        assert_eq!(
            models_list_url(base),
            "https://api.deepseek.com/v1/models"
        );
        assert_eq!(
            chat_completions_url(base),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn glm_urls() {
        let base = "https://open.bigmodel.cn/api/paas/v4";
        assert_eq!(
            models_list_url(base),
            "https://open.bigmodel.cn/api/paas/v4/models"
        );
    }

    #[test]
    fn agens_urls() {
        let base = "https://apihub.agnes-ai.com/v1";
        assert_eq!(
            models_list_url(base),
            "https://apihub.agnes-ai.com/v1/models"
        );
        assert_eq!(
            chat_completions_url(base),
            "https://apihub.agnes-ai.com/v1/chat/completions"
        );
    }

    #[test]
    fn opencode_go_urls() {
        let base = "https://opencode.ai/zen/go/v1";
        assert_eq!(
            models_list_url(base),
            "https://opencode.ai/zen/go/v1/models"
        );
        assert_eq!(
            chat_completions_url(base),
            "https://opencode.ai/zen/go/v1/chat/completions"
        );
    }
}
