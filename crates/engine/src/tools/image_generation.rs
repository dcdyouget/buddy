use super::{Tool, ToolContext, ToolError, ToolOutput, ToolSafety};
use crate::models::ImageAttachment;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::time::Duration;

const OPENAI_MAX_PROMPT_CHARS: usize = 32_000;
const MINIMAX_MAX_PROMPT_CHARS: usize = 1_500;
const ZHIPU_MAX_PROMPT_CHARS: usize = 1_000;
const MINIMAX_IMAGE_MODEL: &str = "image-01";
const ZHIPU_IMAGE_MODEL: &str = "glm-image";
const QWEN_IMAGE_MODEL: &str = "qwen-image-2.0-pro";
const BAIDU_IMAGE_MODEL: &str = "irag-1.0";
const VOLCENGINE_IMAGE_MODEL: &str = "doubao-seedream-5-0-lite-260128";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageApi {
    OpenAi,
    MiniMax,
    Zhipu,
    Qwen,
    Baidu,
    Volcengine,
}

/// 使用当前 Provider 对应的官方生图接口生成图片。
pub struct GenerateImageTool {
    base_url: String,
    api_key: String,
    model_id: String,
    api: ImageApi,
}

impl GenerateImageTool {
    pub fn for_provider(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model_id: impl Into<String>,
        provider_id: &str,
        provider_name: &str,
    ) -> Option<Self> {
        let base_url = base_url.into();
        let model_id = model_id.into();
        let api = detect_image_api(&base_url, provider_id, provider_name, model_id.as_str())?;
        Some(Self {
            base_url,
            api_key: api_key.into(),
            model_id,
            api,
        })
    }

    fn endpoint(&self) -> String {
        if self.api == ImageApi::Qwen {
            return qwen_endpoint(&self.base_url);
        }

        let path = match self.api {
            ImageApi::OpenAi | ImageApi::Zhipu | ImageApi::Baidu | ImageApi::Volcengine => {
                "images/generations"
            }
            ImageApi::MiniMax => "image_generation",
            ImageApi::Qwen => unreachable!(),
        };
        format!("{}/{}", self.base_url.trim().trim_end_matches('/'), path)
    }

    fn generation_model(&self) -> &str {
        match self.api {
            ImageApi::OpenAi => &self.model_id,
            ImageApi::MiniMax => MINIMAX_IMAGE_MODEL,
            ImageApi::Zhipu => ZHIPU_IMAGE_MODEL,
            ImageApi::Qwen => QWEN_IMAGE_MODEL,
            ImageApi::Baidu => BAIDU_IMAGE_MODEL,
            ImageApi::Volcengine => VOLCENGINE_IMAGE_MODEL,
        }
    }

    fn request_body(&self, args: &Value) -> Result<Value, ToolError> {
        let prompt = args
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ToolError::InvalidArgs("缺少非空的 'prompt' 字段".to_string()))?;
        let max_prompt_chars = match self.api {
            ImageApi::OpenAi | ImageApi::Qwen | ImageApi::Baidu | ImageApi::Volcengine => {
                OPENAI_MAX_PROMPT_CHARS
            }
            ImageApi::MiniMax => MINIMAX_MAX_PROMPT_CHARS,
            ImageApi::Zhipu => ZHIPU_MAX_PROMPT_CHARS,
        };
        if prompt.chars().count() > max_prompt_chars {
            return Err(ToolError::InvalidArgs(format!(
                "'prompt' 不能超过 {} 个字符",
                max_prompt_chars
            )));
        }

        if self.api == ImageApi::Qwen {
            let mut parameters = Map::new();
            parameters.insert("n".to_string(), json!(1));
            parameters.insert("prompt_extend".to_string(), json!(true));
            parameters.insert("watermark".to_string(), json!(false));
            if let Some(size) = optional_arg(args, "size") {
                parameters.insert("size".to_string(), json!(size.replace('x', "*")));
            }
            return Ok(json!({
                "model": self.generation_model(),
                "input": {
                    "messages": [{
                        "role": "user",
                        "content": [{"text": prompt}]
                    }]
                },
                "parameters": parameters
            }));
        }

        let mut body = Map::from_iter([
            ("model".to_string(), json!(self.generation_model())),
            ("prompt".to_string(), json!(prompt)),
            ("n".to_string(), json!(1)),
        ]);

        match self.api {
            ImageApi::OpenAi => {
                for key in ["size", "quality"] {
                    if let Some(value) = optional_arg(args, key) {
                        body.insert(key.to_string(), json!(value));
                    }
                }

                // DALL·E 默认返回短时 URL；显式请求 base64，便于本地历史长期展示。
                // GPT Image 模型始终返回 b64_json，不接收该字段。
                if self.model_id.to_ascii_lowercase().starts_with("dall-e") {
                    body.insert("response_format".to_string(), json!("b64_json"));
                }
            }
            ImageApi::MiniMax => {
                body.insert("response_format".to_string(), json!("base64"));
                let aspect_ratio = optional_arg(args, "aspect_ratio")
                    .or_else(|| detect_prompt_aspect_ratio(prompt));
                if let Some(aspect_ratio) = aspect_ratio {
                    body.insert("aspect_ratio".to_string(), json!(aspect_ratio));
                } else if let Some(size) = optional_arg(args, "size") {
                    let (width, height) = size.split_once('x').ok_or_else(|| {
                        ToolError::InvalidArgs(format!("不支持的图片尺寸：{size}"))
                    })?;
                    let width = width
                        .parse::<u16>()
                        .map_err(|_| ToolError::InvalidArgs(format!("不支持的图片尺寸：{size}")))?;
                    let height = height
                        .parse::<u16>()
                        .map_err(|_| ToolError::InvalidArgs(format!("不支持的图片尺寸：{size}")))?;
                    body.insert("width".to_string(), json!(width));
                    body.insert("height".to_string(), json!(height));
                }
            }
            ImageApi::Zhipu | ImageApi::Baidu | ImageApi::Volcengine => {
                if let Some(size) = optional_arg(args, "size") {
                    body.insert("size".to_string(), json!(size));
                }
            }
            ImageApi::Qwen => unreachable!(),
        }

        Ok(Value::Object(body))
    }

    fn parse_response(
        &self,
        payload: &str,
        original_prompt: &str,
    ) -> Result<ToolOutput, ToolError> {
        match self.api {
            ImageApi::OpenAi | ImageApi::Zhipu | ImageApi::Baidu | ImageApi::Volcengine => {
                parse_openai_response(payload, self.generation_model(), original_prompt)
            }
            ImageApi::MiniMax => {
                parse_minimax_response(payload, self.generation_model(), original_prompt)
            }
            ImageApi::Qwen => {
                parse_qwen_response(payload, self.generation_model(), original_prompt)
            }
        }
    }
}

fn optional_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "auto")
}

fn detect_prompt_aspect_ratio(prompt: &str) -> Option<&'static str> {
    ["21:9", "16:9", "9:16", "4:3", "3:4", "3:2", "2:3", "1:1"]
        .into_iter()
        .find(|ratio| prompt.contains(ratio))
}

fn detect_image_api(
    base_url: &str,
    provider_id: &str,
    provider_name: &str,
    model_id: &str,
) -> Option<ImageApi> {
    let labels = format!("{provider_id} {provider_name}").to_ascii_lowercase();
    let model = model_id.to_ascii_lowercase();
    let host = reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default();

    // Kimi 与 DeepSeek 目前只有视觉理解，没有可复用当前 API Key 的公开生图 API。
    if labels.contains("moonshot")
        || labels.contains("kimi")
        || labels.contains("月之暗面")
        || model.contains("kimi")
        || host_matches(&host, "moonshot.cn")
        || host_matches(&host, "moonshot.ai")
        || labels.contains("deepseek")
        || model.contains("deepseek")
        || host_matches(&host, "deepseek.com")
    {
        return None;
    }

    if labels.contains("minimax")
        || host_matches(&host, "minimax.io")
        || host_matches(&host, "minimaxi.com")
        || host_matches(&host, "minimax.chat")
    {
        Some(ImageApi::MiniMax)
    } else if labels.contains("zhipu")
        || labels.contains("智谱")
        || labels.split_whitespace().any(|label| label == "glm")
        || host_matches(&host, "bigmodel.cn")
    {
        Some(ImageApi::Zhipu)
    } else if labels.contains("qwen")
        || labels.contains("通义")
        || labels.contains("百炼")
        || host_matches(&host, "dashscope.aliyuncs.com")
        || host_matches(&host, "maas.aliyuncs.com")
    {
        Some(ImageApi::Qwen)
    } else if labels.contains("qianfan")
        || labels.contains("百度")
        || host_matches(&host, "qianfan.baidubce.com")
    {
        Some(ImageApi::Baidu)
    } else if labels.contains("doubao")
        || labels.contains("豆包")
        || labels.contains("volcengine")
        || labels.contains("火山")
        || host_matches(&host, "volces.com")
    {
        Some(ImageApi::Volcengine)
    } else {
        Some(ImageApi::OpenAi)
    }
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn qwen_endpoint(base_url: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(base_url.trim()) else {
        return format!(
            "{}/api/v1/services/aigc/multimodal-generation/generation",
            base_url.trim().trim_end_matches('/')
        );
    };
    url.set_path("/api/v1/services/aigc/multimodal-generation/generation");
    url.set_query(None);
    url.set_fragment(None);
    url.to_string().trim_end_matches('/').to_string()
}

fn build_output(
    model_id: &str,
    original_prompt: &str,
    images: Vec<ImageAttachment>,
    revised_prompts: Vec<String>,
) -> Result<ToolOutput, ToolError> {
    if images.is_empty() {
        return Err(ToolError::Other(
            "图片生成接口未返回可用的图片数据".to_string(),
        ));
    }

    let content = serde_json::to_string(&json!({
        "status": "ok",
        "model": model_id,
        "prompt": original_prompt,
        "image_count": images.len(),
        "revised_prompts": revised_prompts,
        "note": "图片已生成并展示给用户。不要在回答中输出 base64 或重复生成。"
    }))?;

    Ok(ToolOutput::ok(content).with_images(images))
}

fn image_attachment(
    index: usize,
    data_url: String,
    media_type: &str,
    extension: &str,
) -> ImageAttachment {
    ImageAttachment {
        id: format!(
            "generated-{}-{}",
            chrono::Utc::now().timestamp_millis(),
            index
        ),
        name: format!("generated-{}.{}", index + 1, extension),
        media_type: media_type.to_string(),
        path: String::new(),
        data_url,
    }
}

#[derive(Debug, Deserialize)]
struct ApiImage {
    #[serde(default)]
    b64_json: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    revised_prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageGenerationResponse {
    #[serde(default)]
    data: Vec<ApiImage>,
}

/// 从图片字节嗅探真实媒体类型；识别不出时返回 None。
fn sniff_image_media_type(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(b"GIF8") {
        Some(("image/gif", "gif"))
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    }
}

/// 用 base64 前缀（前 16 字符 ≈ 12 字节）嗅探真实图片格式，
/// 避免把所有 OpenAI 家族响应都硬标成 image/png（可能是 JPEG/WebP）。
fn sniff_base64_media_type(base64: &str) -> Option<(&'static str, &'static str)> {
    let head = base64.get(..16)?;
    let bytes = BASE64_STANDARD.decode(head).ok()?;
    sniff_image_media_type(&bytes)
}

/// 图片地址只接受 HTTPS（与下载路径的校验一致）。
fn is_https_image_url(url: &str) -> bool {
    url.starts_with("https://")
}

/// 等待取消信号；无信号源（None）时永不完成。
async fn wait_for_cancel(cancel_rx: &mut Option<tokio::sync::watch::Receiver<bool>>) {
    if let Some(rx) = cancel_rx.as_mut() {
        let _ = rx.changed().await;
    } else {
        std::future::pending::<()>().await;
    }
}

fn parse_openai_response(
    payload: &str,
    model_id: &str,
    original_prompt: &str,
) -> Result<ToolOutput, ToolError> {
    let response: OpenAiImageGenerationResponse = serde_json::from_str(payload)
        .map_err(|error| ToolError::Other(format!("图片生成响应解析失败: {}", error)))?;

    let mut images = Vec::new();
    let mut revised_prompts = Vec::new();
    for (index, item) in response.data.into_iter().enumerate() {
        let b64 = item.b64_json.filter(|value| !value.trim().is_empty());
        let url = item.url.filter(|value| !value.trim().is_empty());
        let attachment = match (b64, url) {
            (Some(base64), _) => {
                // 按真实字节嗅探媒体类型，而不是硬编码 image/png
                let (media_type, extension) =
                    sniff_base64_media_type(&base64).unwrap_or(("image/png", "png"));
                image_attachment(
                    index,
                    format!("data:{media_type};base64,{}", base64),
                    media_type,
                    extension,
                )
            }
            (None, Some(url)) => {
                // 只接受 HTTPS 图片地址，避免任意协议/内网 URL 进入渲染与下载流程
                if !is_https_image_url(&url) {
                    continue;
                }
                image_attachment(index, url, "image/png", "png")
            }
            (None, None) => continue,
        };

        if let Some(revised) = item.revised_prompt.filter(|value| !value.trim().is_empty()) {
            revised_prompts.push(revised);
        }
        images.push(attachment);
    }

    build_output(model_id, original_prompt, images, revised_prompts)
}

#[derive(Debug, Default, Deserialize)]
struct MiniMaxImageData {
    #[serde(default)]
    image_base64: Vec<String>,
    #[serde(default)]
    image_urls: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MiniMaxBaseResponse {
    #[serde(default)]
    status_code: i64,
    #[serde(default)]
    status_msg: String,
}

#[derive(Debug, Deserialize)]
struct MiniMaxImageGenerationResponse {
    #[serde(default)]
    data: Option<MiniMaxImageData>,
    #[serde(default)]
    base_resp: MiniMaxBaseResponse,
}

fn minimax_status_code(payload: &str) -> Option<i64> {
    serde_json::from_str::<MiniMaxImageGenerationResponse>(payload)
        .ok()
        .map(|response| response.base_resp.status_code)
}

fn is_retryable_minimax_status(status_code: i64) -> bool {
    matches!(status_code, 1000 | 1001 | 1002 | 1013 | 1024 | 1033 | 1039)
}

fn parse_minimax_response(
    payload: &str,
    model_id: &str,
    original_prompt: &str,
) -> Result<ToolOutput, ToolError> {
    let response: MiniMaxImageGenerationResponse = serde_json::from_str(payload)
        .map_err(|error| ToolError::Other(format!("MiniMax 图片生成响应解析失败: {}", error)))?;
    if response.base_resp.status_code != 0 {
        let message = if response.base_resp.status_msg.trim().is_empty() {
            format!("状态码 {}", response.base_resp.status_code)
        } else {
            response.base_resp.status_msg
        };
        return Err(ToolError::Other(format!("MiniMax 图片生成失败: {message}")));
    }

    let data = response.data.unwrap_or_default();
    let mut images = Vec::new();
    for base64 in data
        .image_base64
        .into_iter()
        .filter(|value| !value.trim().is_empty())
    {
        let index = images.len();
        images.push(image_attachment(
            index,
            format!("data:image/jpeg;base64,{base64}"),
            "image/jpeg",
            "jpg",
        ));
    }
    for url in data
        .image_urls
        .into_iter()
        .filter(|value| !value.trim().is_empty() && is_https_image_url(value))
    {
        let index = images.len();
        images.push(image_attachment(index, url, "image/jpeg", "jpg"));
    }

    build_output(model_id, original_prompt, images, Vec::new())
}

fn parse_qwen_response(
    payload: &str,
    model_id: &str,
    original_prompt: &str,
) -> Result<ToolOutput, ToolError> {
    let response: Value = serde_json::from_str(payload)
        .map_err(|error| ToolError::Other(format!("Qwen 图片生成响应解析失败: {}", error)))?;
    let content = response
        .pointer("/output/choices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|choice| choice.pointer("/message/content").and_then(Value::as_array))
        .flatten();

    let images = content
        .filter_map(|item| item.get("image").and_then(Value::as_str))
        .map(str::trim)
        .filter(|url| !url.is_empty() && is_https_image_url(url))
        .enumerate()
        .map(|(index, url)| image_attachment(index, url.to_string(), "image/png", "png"))
        .collect();

    build_output(model_id, original_prompt, images, Vec::new())
}

fn extract_api_error(payload: &str) -> String {
    serde_json::from_str::<Value>(payload)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
                .or_else(|| value.pointer("/base_resp/status_msg"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| payload.chars().take(500).collect())
}

#[async_trait]
impl Tool for GenerateImageTool {
    fn name(&self) -> &str {
        "generate_image"
    }

    fn description(&self) -> &str {
        "根据用户要求生成一张图片。仅当用户明确要求创建、绘制或生成图片时调用；不要用于识图。工具会调用当前 Provider 对应的官方图片生成接口并把图片直接展示给用户。prompt 应完整描述主体、场景、构图、风格、光线与文字要求；用户指定宽高比时必须填写 aspect_ratio。工具内部会处理瞬时错误重试；如果调用仍失败，同一轮不要再次调用，直接说明失败原因。"
    }

    fn parameters_schema(&self) -> Value {
        // 不同厂商对 prompt 长度上限不同：按当前 API 类型公布正确的上限，
        // 避免模型生成超长 prompt 被 MiniMax(1500)/Zhipu(1000) 拒绝。
        let max_prompt_chars = match self.api {
            ImageApi::MiniMax => MINIMAX_MAX_PROMPT_CHARS,
            ImageApi::Zhipu => ZHIPU_MAX_PROMPT_CHARS,
            _ => OPENAI_MAX_PROMPT_CHARS,
        };
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "maxLength": max_prompt_chars,
                    "description": "用于生成图片的完整提示词"
                },
                "size": {
                    "type": "string",
                    "enum": [
                        "auto",
                        "1024x1024",
                        "1024x1536",
                        "1536x1024",
                        "1024x1792",
                        "1792x1024"
                    ],
                    "description": "可选图片尺寸；用户未指定时使用 auto"
                },
                "aspect_ratio": {
                    "type": "string",
                    "enum": ["1:1", "16:9", "4:3", "3:2", "2:3", "3:4", "9:16", "21:9"],
                    "description": "可选宽高比；用户明确要求横版、竖版或具体比例时必须填写"
                },
                "quality": {
                    "type": "string",
                    "enum": ["auto", "low", "medium", "high", "standard", "hd"],
                    "description": "可选图片质量；用户未指定时使用 auto"
                }
            },
            "required": ["prompt"]
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }

    async fn execute(&self, args: Value, mut ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let prompt = args
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        let body = self.request_body(&args)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|error| ToolError::Other(format!("创建图片生成客户端失败: {}", error)))?;

        // 用户点击"停止生成"后立即中止（生图请求最长 180s，不能继续等待/消耗额度）
        let max_attempts = if self.api == ImageApi::MiniMax { 2 } else { 1 };
        for attempt in 0..max_attempts {
            if ctx.is_cancelled() {
                return Err(ToolError::Other("用户已取消图片生成".to_string()));
            }
            let response = match tokio::select! {
                r = client
                    .post(self.endpoint())
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send() => r,
                _ = wait_for_cancel(&mut ctx.cancel_rx) => {
                    // 停止信号触发：watch 只发 true，到达即为取消
                    return Err(ToolError::Other("用户已取消图片生成".to_string()));
                }
            } {
                Ok(response) => response,
                Err(error)
                    if attempt + 1 < max_attempts && (error.is_timeout() || error.is_connect()) =>
                {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(error) => {
                    return Err(ToolError::Other(format!("图片生成请求失败: {}", error)));
                }
            };

            let status = response.status();
            let payload = tokio::select! {
                t = response.text() => t,
                _ = wait_for_cancel(&mut ctx.cancel_rx) => {
                    return Err(ToolError::Other("用户已取消图片生成".to_string()));
                }
            }
            .map_err(|error| ToolError::Other(format!("读取图片生成响应失败: {}", error)))?;
            let should_retry = attempt + 1 < max_attempts
                && (status.as_u16() == 429
                    || status.is_server_error()
                    || (self.api == ImageApi::MiniMax
                        && minimax_status_code(&payload).is_some_and(is_retryable_minimax_status)));
            if should_retry {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            if !status.is_success() {
                return Err(ToolError::Other(format!(
                    "图片生成接口返回 HTTP {}: {}",
                    status.as_u16(),
                    extract_api_error(&payload)
                )));
            }

            return self.parse_response(&payload, &prompt);
        }

        Err(ToolError::Other("图片生成重试后仍然失败".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn tool(model_id: &str) -> GenerateImageTool {
        GenerateImageTool::for_provider(
            "https://api.example.com/v1/",
            "secret",
            model_id,
            "example",
            "Example",
        )
        .unwrap()
    }

    fn minimax_tool() -> GenerateImageTool {
        GenerateImageTool::for_provider(
            "https://api.minimaxi.com/v1/",
            "secret",
            "MiniMax-M3",
            "minimax",
            "MiniMax",
        )
        .unwrap()
    }

    #[test]
    fn builds_openai_image_endpoint_and_body() {
        let tool = tool("gpt-image-2");
        assert_eq!(
            tool.endpoint(),
            "https://api.example.com/v1/images/generations"
        );
        let body = tool
            .request_body(&json!({
                "prompt": "一只猫",
                "size": "1024x1024",
                "quality": "auto"
            }))
            .unwrap();
        assert_eq!(body["model"], "gpt-image-2");
        assert_eq!(body["prompt"], "一只猫");
        assert_eq!(body["n"], 1);
        assert_eq!(body["size"], "1024x1024");
        assert!(body.get("quality").is_none());
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn dalle_requests_persistable_base64() {
        let body = tool("dall-e-3")
            .request_body(&json!({ "prompt": "cat" }))
            .unwrap();
        assert_eq!(body["response_format"], "b64_json");
    }

    #[test]
    fn parses_base64_and_url_images_without_returning_bytes_to_model() {
        let output = parse_openai_response(
            r#"{"data":[{"b64_json":"aGVsbG8="},{"url":"https://example.com/a.png"}]}"#,
            "gpt-image-2",
            "cat",
        )
        .unwrap();
        assert_eq!(output.images.len(), 2);
        assert!(output.images[0]
            .data_url
            .starts_with("data:image/png;base64,"));
        assert_eq!(output.images[1].data_url, "https://example.com/a.png");
        assert!(!output.content.contains("aGVsbG8="));
    }

    #[test]
    fn rejects_empty_prompt_and_empty_image_data() {
        assert!(matches!(
            tool("gpt-image-2").request_body(&json!({ "prompt": " " })),
            Err(ToolError::InvalidArgs(_))
        ));
        assert!(parse_openai_response(r#"{"data":[{}]}"#, "gpt-image-2", "cat").is_err());
    }

    #[test]
    fn builds_minimax_native_endpoint_and_body() {
        let tool = minimax_tool();
        assert_eq!(
            tool.endpoint(),
            "https://api.minimaxi.com/v1/image_generation"
        );
        assert_eq!(tool.generation_model(), "image-01");

        let body = tool
            .request_body(&json!({
                "prompt": "一只猫",
                "size": "1024x1536",
                "quality": "high"
            }))
            .unwrap();
        assert_eq!(body["model"], "image-01");
        assert_eq!(body["prompt"], "一只猫");
        assert_eq!(body["n"], 1);
        assert_eq!(body["response_format"], "base64");
        assert_eq!(body["width"], 1024);
        assert_eq!(body["height"], 1536);
        assert!(body.get("quality").is_none());
    }

    #[test]
    fn minimax_prefers_explicit_or_prompt_aspect_ratio_over_size() {
        let tool = minimax_tool();
        let explicit = tool
            .request_body(&json!({
                "prompt": "横版机器人",
                "aspect_ratio": "16:9",
                "size": "1536x1024"
            }))
            .unwrap();
        assert_eq!(explicit["aspect_ratio"], "16:9");
        assert!(explicit.get("width").is_none());
        assert!(explicit.get("height").is_none());

        let inferred = tool
            .request_body(&json!({
                "prompt": "生成一张 16:9 的机器人",
                "size": "1536x1024"
            }))
            .unwrap();
        assert_eq!(inferred["aspect_ratio"], "16:9");
        assert!(inferred.get("width").is_none());
    }

    #[test]
    fn detects_supported_domestic_providers_and_blocks_unsupported_ones() {
        let cases = [
            (
                "https://api.minimax.chat/v1",
                "custom",
                "自定义",
                "MiniMax-M3",
                Some(ImageApi::MiniMax),
            ),
            (
                "https://open.bigmodel.cn/api/paas/v4",
                "custom",
                "自定义",
                "glm-4.5",
                Some(ImageApi::Zhipu),
            ),
            (
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "custom",
                "自定义",
                "qwen3",
                Some(ImageApi::Qwen),
            ),
            (
                "https://qianfan.baidubce.com/v2",
                "custom",
                "自定义",
                "ernie-4.5",
                Some(ImageApi::Baidu),
            ),
            (
                "https://ark.cn-beijing.volces.com/api/v3",
                "custom",
                "自定义",
                "doubao-seed",
                Some(ImageApi::Volcengine),
            ),
            (
                "https://api.moonshot.cn/v1",
                "moonshot",
                "Moonshot / 月之暗面",
                "kimi-k2.5",
                None,
            ),
            (
                "https://api.deepseek.com",
                "deepseek",
                "DeepSeek",
                "deepseek-chat",
                None,
            ),
        ];

        for (base_url, provider_id, provider_name, model_id, expected) in cases {
            assert_eq!(
                detect_image_api(base_url, provider_id, provider_name, model_id),
                expected
            );
        }
    }

    #[test]
    fn builds_domestic_provider_endpoints_models_and_bodies() {
        let zhipu = GenerateImageTool::for_provider(
            "https://open.bigmodel.cn/api/paas/v4",
            "secret",
            "glm-4.5",
            "glm",
            "GLM / 智谱",
        )
        .unwrap();
        assert_eq!(
            zhipu.endpoint(),
            "https://open.bigmodel.cn/api/paas/v4/images/generations"
        );
        assert_eq!(zhipu.generation_model(), "glm-image");

        let qwen = GenerateImageTool::for_provider(
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "secret",
            "qwen3-max",
            "qwen",
            "Qwen / 通义千问",
        )
        .unwrap();
        assert_eq!(
            qwen.endpoint(),
            "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation"
        );
        let qwen_body = qwen
            .request_body(&json!({"prompt": "山水画", "size": "1024x1536"}))
            .unwrap();
        assert_eq!(qwen_body["model"], "qwen-image-2.0-pro");
        assert_eq!(
            qwen_body["input"]["messages"][0]["content"][0]["text"],
            "山水画"
        );
        assert_eq!(qwen_body["parameters"]["size"], "1024*1536");

        let baidu = GenerateImageTool::for_provider(
            "https://qianfan.baidubce.com/v2",
            "secret",
            "ernie-4.5",
            "qianfan",
            "百度千帆",
        )
        .unwrap();
        assert_eq!(
            baidu.endpoint(),
            "https://qianfan.baidubce.com/v2/images/generations"
        );
        assert_eq!(baidu.generation_model(), "irag-1.0");

        let doubao = GenerateImageTool::for_provider(
            "https://ark.cn-beijing.volces.com/api/v3",
            "secret",
            "doubao-seed-1.6",
            "doubao",
            "豆包 / 火山方舟",
        )
        .unwrap();
        assert_eq!(
            doubao.endpoint(),
            "https://ark.cn-beijing.volces.com/api/v3/images/generations"
        );
        assert_eq!(doubao.generation_model(), "doubao-seedream-5-0-lite-260128");
    }

    #[test]
    fn parses_qwen_nested_image_response() {
        let output = parse_qwen_response(
            r#"{
                "output": {
                    "choices": [{
                        "message": {
                            "content": [{"image": "https://example.com/qwen.png"}]
                        }
                    }]
                }
            }"#,
            QWEN_IMAGE_MODEL,
            "山水画",
        )
        .unwrap();
        assert_eq!(output.images.len(), 1);
        assert_eq!(output.images[0].data_url, "https://example.com/qwen.png");
    }

    #[test]
    fn parses_minimax_base64_and_url_images() {
        let output = parse_minimax_response(
            r#"{
                "data": {
                    "image_base64": ["aGVsbG8="],
                    "image_urls": ["https://example.com/minimax.jpg"]
                },
                "base_resp": {"status_code": 0, "status_msg": "success"}
            }"#,
            "image-01",
            "cat",
        )
        .unwrap();
        assert_eq!(output.images.len(), 2);
        assert!(output.images[0]
            .data_url
            .starts_with("data:image/jpeg;base64,"));
        assert_eq!(output.images[1].data_url, "https://example.com/minimax.jpg");
        assert!(output.content.contains(r#""model":"image-01""#));
        assert!(!output.content.contains("aGVsbG8="));
    }

    #[test]
    fn reports_minimax_business_error() {
        let error = parse_minimax_response(
            r#"{
                "data": {},
                "base_resp": {"status_code": 1004, "status_msg": "invalid api key"}
            }"#,
            "image-01",
            "cat",
        )
        .unwrap_err();
        assert!(error.to_string().contains("invalid api key"));
    }

    #[test]
    fn reports_minimax_null_data_business_error_instead_of_parse_error() {
        let error = parse_minimax_response(
            r#"{
                "data": null,
                "base_resp": {"status_code": 1002, "status_msg": "rate limit"}
            }"#,
            "image-01",
            "cat",
        )
        .unwrap_err();
        assert!(error.to_string().contains("rate limit"));
        assert!(!error.to_string().contains("响应解析失败"));
        assert!(is_retryable_minimax_status(1002));
        assert!(!is_retryable_minimax_status(1004));
    }

    #[tokio::test]
    #[ignore = "requires local TCP listener"]
    async fn executes_openai_image_generation_request_end_to_end() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }

            let body = r#"{"data":[{"b64_json":"aGVsbG8="}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(request).unwrap()
        });

        let tool = GenerateImageTool::for_provider(
            format!("http://{}/v1", address),
            "test-key",
            "gpt-image-2",
            "example",
            "Example",
        )
        .unwrap();
        let output = tool
            .execute(
                json!({ "prompt": "一只猫", "size": "1024x1024" }),
                ToolContext::default(),
            )
            .await
            .unwrap();
        let request = server.await.unwrap();

        assert!(request.starts_with("POST /v1/images/generations HTTP/1.1"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-key"));
        assert!(request.contains(r#""model":"gpt-image-2""#));
        assert!(request.contains(r#""prompt":"一只猫""#));
        assert_eq!(output.images.len(), 1);
        assert!(!output.is_error);
    }
}
