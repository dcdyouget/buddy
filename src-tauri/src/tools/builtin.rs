// ============================================================================
// 内置 file tool 实现
// ============================================================================
//
// 基础文件 tool:
//   - read_file      (ReadOnly) — 读取文本文件内容
//   - create_file    (Write)    — 创建新文件,目标必须不存在
//   - overwrite_file (Write)    — 覆盖现有文件,目标必须存在
//   - append_file    (Write)    — 追加或创建
// 目录浏览、文件搜索与局部编辑见 file_tools.rs。
//
// 路径安全(Q3 决策):write 类必须以 AppConfig.allowed_paths 某条为前缀;
//                   allowed_paths 为空时不限制;
//                   read 类不做路径限制。
//
// 路径规范:
//   - 用户传入的 path 可以是绝对路径或相对路径
//   - 相对路径相对 cwd 处理(注:buddy 是 Tauri 应用,启动时 cwd 是 app bundle 目录,
//     实际使用中 model 通常传绝对路径,这里按"原样使用"处理,不强制 canonicalize)
//   - 符号链接防护:写入前解析"最近已存在祖先"的真实路径再校验一次白名单,
//     阻止 allowed_paths 目录内指向白名单外的符号链接(写入时仍直接操作原路径,
//     不额外引入 check-then-act 的 TOCTOU 面)
// ============================================================================

use super::{Tool, ToolContext, ToolError, ToolOutput, ToolSafety};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// 覆盖写临时文件的全局序号：保证并发覆盖同一目标时临时文件名不冲突。
static OVERWRITE_TMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// 写工具的统一路径校验：先做词法白名单校验；allowed_paths 非空时，
/// 再解析「路径上最近已存在祖先」的真实路径并复检一次，堵住符号链接逃逸。
pub(super) fn check_write_allowed_with_symlinks(
    path: &Path,
    allowed_paths: &[String],
) -> Result<(), ToolError> {
    // 词法白名单（快路径）
    check_write_allowed(path, allowed_paths)?;
    if allowed_paths.is_empty() {
        return Ok(());
    }
    // 解析目标路径的符号链接后再校验一次，堵住白名单目录内指向外部的软链。
    let Some(resolved) = canonicalize_nearest_existing(path) else {
        return Ok(());
    };
    if resolved == normalize_path(path) {
        return Ok(()); // 无符号链接差异，词法校验已足够
    }
    // 与「规范化后的白名单前缀」比较：macOS 上 /var → /private/var 这类系统软链
    // 会让 canonicalize 结果与配置里的字面量不一致，因此白名单两侧都要 canonicalize。
    for allowed in allowed_paths {
        if allowed.trim().is_empty() {
            continue;
        }
        let allowed_path = Path::new(allowed);
        let normalized = normalize_path(allowed_path);
        let canonical = std::fs::canonicalize(allowed_path)
            .unwrap_or_else(|_| normalized.clone());
        if resolved == normalized
            || resolved.starts_with(&normalized)
            || resolved == canonical
            || resolved.starts_with(&canonical)
        {
            return Ok(());
        }
    }
    Err(ToolError::PermissionDenied(format!(
        "路径 {} 不在 allowed_paths 白名单中",
        path.display()
    )))
}

/// 解析「路径上最近已存在的祖先」的符号链接，再拼回尚未创建的部分。
fn canonicalize_nearest_existing(path: &Path) -> Option<PathBuf> {
    let mut ancestor = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match std::fs::canonicalize(&ancestor) {
            Ok(mut resolved) => {
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Some(resolved);
            }
            Err(_) => match ancestor.file_name() {
                Some(name) => {
                    missing.push(name.to_os_string());
                    ancestor = ancestor.parent()?.to_path_buf();
                }
                None => return None,
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 路径检查工具
// ─────────────────────────────────────────────────────────────────────────────

/// 检查 write 类路径是否在 allowed_paths 白名单内
///
/// allowed_paths 为空 → 返回 Ok(不限制)
/// 否则要求 path 至少有一条前缀匹配(prefix match,不要求完全相等)
pub(super) fn check_write_allowed(path: &Path, allowed_paths: &[String]) -> Result<(), ToolError> {
    if allowed_paths.is_empty() {
        return Ok(());
    }

    let normalized_path = normalize_path(path);

    for allowed in allowed_paths {
        if allowed.trim().is_empty() {
            continue;
        }
        let normalized_allowed = normalize_path(Path::new(allowed));
        if normalized_path == normalized_allowed || normalized_path.starts_with(&normalized_allowed)
        {
            return Ok(());
        }
    }

    Err(ToolError::PermissionDenied(format!(
        "路径 {} 不在 allowed_paths 白名单中",
        path.display()
    )))
}

/// 只做词法规范化,消除 `.` 与 `..`,避免通过父目录片段绕过路径白名单。
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !path.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
        }
    }
    normalized
}

// ─────────────────────────────────────────────────────────────────────────────
// 公共 schema 辅助
// ─────────────────────────────────────────────────────────────────────────────

fn path_property() -> Value {
    json!({
        "type": "string",
        "description": "目标文件路径,绝对路径(如 /Users/me/foo.txt)或相对路径"
    })
}

fn content_property() -> Value {
    json!({
        "type": "string",
        "description": "要写入的完整内容"
    })
}

/// 抽取 args.path 字段
fn extract_path(args: &Value) -> Result<PathBuf, ToolError> {
    let p = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArgs("缺少 'path' 字段或不是字符串".to_string()))?;
    if p.is_empty() {
        return Err(ToolError::InvalidArgs("'path' 不能为空".to_string()));
    }
    Ok(PathBuf::from(p))
}

/// 抽取 args.content 字段(若缺失/非 string 返回空串)
fn extract_content(args: &Value) -> String {
    args.get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// read_file
// ─────────────────────────────────────────────────────────────────────────────

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "读取本地文本文件的内容。返回 UTF-8 解码后的文本;二进制文件返回 is_error。"
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": path_property()
            },
            "required": ["path"]
        })
    }
    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let path = extract_path(&args)?;
        let content = fs::read_to_string(&path).await.map_err(|e| {
            ToolError::Io(match e.kind() {
                std::io::ErrorKind::NotFound => std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("文件不存在: {}", path.display()),
                ),
                _ => e,
            })
        })?;
        Ok(ToolOutput::ok(content))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// create_file
// ─────────────────────────────────────────────────────────────────────────────

pub struct CreateFileTool {
    pub allowed_paths: Vec<String>,
}

#[async_trait]
impl Tool for CreateFileTool {
    fn name(&self) -> &str {
        "create_file"
    }
    fn description(&self) -> &str {
        "创建新文件。目标文件必须不存在,存在则报错。仅在 allowed_paths 白名单内允许。"
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": path_property(),
                "content": content_property()
            },
            "required": ["path", "content"]
        })
    }
    fn safety(&self) -> ToolSafety {
        ToolSafety::Write
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let path = extract_path(&args)?;
        check_write_allowed_with_symlinks(&path, &self.allowed_paths)?;

        // 自动创建父目录
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).await?;
            }
        }

        let content = extract_content(&args);
        // create_new(true)：原子地"仅当不存在时创建"，避免检查-创建之间的 TOCTOU 覆盖已有文件
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    ToolError::InvalidArgs(format!(
                        "文件已存在: {} (请用 overwrite_file 或 append_file)",
                        path.display()
                    ))
                } else {
                    ToolError::Io(e)
                }
            })?;
        f.write_all(content.as_bytes()).await?;
        f.sync_all().await?;

        Ok(ToolOutput::ok(format!(
            "已创建文件: {} ({} 字节)",
            path.display(),
            content.len()
        )))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// overwrite_file
// ─────────────────────────────────────────────────────────────────────────────

pub struct OverwriteFileTool {
    pub allowed_paths: Vec<String>,
}

#[async_trait]
impl Tool for OverwriteFileTool {
    fn name(&self) -> &str {
        "overwrite_file"
    }
    fn description(&self) -> &str {
        "覆盖现有文件。目标文件必须存在,不存在则报错。仅在 allowed_paths 白名单内允许。"
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": path_property(),
                "content": content_property()
            },
            "required": ["path", "content"]
        })
    }
    fn safety(&self) -> ToolSafety {
        ToolSafety::Write
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let path = extract_path(&args)?;
        check_write_allowed_with_symlinks(&path, &self.allowed_paths)?;

        if fs::metadata(&path).await.is_err() {
            return Err(ToolError::InvalidArgs(format!(
                "文件不存在: {} (请用 create_file 或 append_file)",
                path.display()
            )));
        }

        let content = extract_content(&args);
        // 唯一临时名：并发覆盖同一目标时互不冲突
        let tmp = path.with_extension(format!(
            "tmp_buddy_overwrite_{}_{}",
            std::process::id(),
            OVERWRITE_TMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        {
            let mut f = fs::File::create(&tmp).await?;
            f.write_all(content.as_bytes()).await?;
            f.sync_all().await?;
        }
        // 原子 rename;失败时清理 tmp
        if let Err(e) = fs::rename(&tmp, &path).await {
            let _ = fs::remove_file(&tmp).await;
            return Err(ToolError::Io(e));
        }

        Ok(ToolOutput::ok(format!(
            "已覆盖文件: {} ({} 字节)",
            path.display(),
            content.len()
        )))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// append_file
// ─────────────────────────────────────────────────────────────────────────────

pub struct AppendFileTool {
    pub allowed_paths: Vec<String>,
}

#[async_trait]
impl Tool for AppendFileTool {
    fn name(&self) -> &str {
        "append_file"
    }
    fn description(&self) -> &str {
        "追加内容到现有文件,或创建新文件。仅在 allowed_paths 白名单内允许。"
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": path_property(),
                "content": content_property()
            },
            "required": ["path", "content"]
        })
    }
    fn safety(&self) -> ToolSafety {
        ToolSafety::Write
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let path = extract_path(&args)?;
        check_write_allowed_with_symlinks(&path, &self.allowed_paths)?;

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).await?;
            }
        }

        let existed = fs::metadata(&path).await.is_ok();
        let content = extract_content(&args);

        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        f.write_all(content.as_bytes()).await?;
        f.sync_all().await?;

        Ok(ToolOutput::ok(format!(
            "{}文件: {} (追加 {} 字节)",
            if existed { "已追加到" } else { "已创建" },
            path.display(),
            content.len()
        )))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ask_user — 让模型可以向用户提出选择题
// ─────────────────────────────────────────────────────────────────────────────
//
// 这个 tool 不在本地真正"执行"任何文件操作 — 它的作用是阻塞当前 turn 的
// 工具循环,直到前端用户在内联问答卡里做出选择。命令 (commands.rs) 中
// 识别到 ask_user 调用时,会:
//   1. 发射 ToolQuestionRequired 事件(让前端显示内联问答卡)
//   2. 等待 answer_tool_question 命令 invoke,把答案作为 tool result 写回
//
// 因此 execute() 永远不会被调用 — 如果走到这里,说明调用链出错,
// 返回 error 让 model 看到失败。
//
// ToolSafety 用 ReadOnly — 走完正常 tool 循环不会被 ApprovalModal 拦截,
// 由 commands.rs 里的专门分支处理。

/// ask_user 的参数 schema(JSON,直接发给 LLM)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskUserArgs {
    /// 要问用户的问题
    pub question: String,
    /// 2-4 个互斥选项
    pub options: Vec<AskUserOption>,
    /// 是否允许多选(默认 false)
    #[serde(default)]
    pub multi_select: bool,
    /// 短标签(显示在 chip 上,最长 12 字符)
    pub header: String,
}

/// ask_user 的单个选项
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskUserOption {
    /// 1-5 词的简短标签
    pub label: String,
    /// 可选说明
    #[serde(default)]
    pub description: String,
    /// 此选项是否需要用户补充输入(如"选其他文件"时让用户输入路径)
    #[serde(default)]
    pub requires_input: bool,
    /// 输入框的占位符(仅在 requires_input=true 时生效)
    #[serde(default)]
    pub input_placeholder: String,
}

/// 用户对 ask_user 的回答(answer_tool_question 命令接收)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskUserAnswer {
    /// 用户选择的选项索引(单选时取 [0],多选时可能多个)
    pub selected: Vec<usize>,
    /// 对应 selected 中每个选项的补充输入(空字符串表示未填)
    /// Vec 长度与 selected 相同;只有 requires_input=true 的选项需要非空值
    #[serde(default)]
    pub inputs: Vec<String>,
    /// 用户输入的自定义回答(如果 model 允许且用户没用预设选项)
    #[serde(default)]
    pub custom: Option<String>,
}

pub struct AskUserTool;

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn description(&self) -> &str {
        "Ask the user a clarifying question with 2-4 mutually exclusive multiple-choice options. \
         Use this whenever you encounter a choice point that affects the user's outcome \
         (e.g. \"file already exists, overwrite or cancel?\"). \
         Each option label must be 1-5 words. Set multi_select=true only if the choices are \
         independent. Do NOT use this for simple yes/no questions — just ask in plain text. \
         Do NOT use this when you can make a reasonable default choice on your own.\n\n\
         For options that need extra info (e.g. \"use a different file path\"), set \
         requires_input=true and provide input_placeholder. The UI will show a text input below \
         that option and require it to be filled before the user can submit. Typical use: \
         an \"Other / specify...\" option with requires_input=true."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to present to the user, written in the same language as the conversation"
                },
                "header": {
                    "type": "string",
                    "description": "Very short label (max 12 chars), shown as a chip/tag on the question"
                },
                "options": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": 4,
                    "description": "2-4 options for the user to choose from",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": {
                                "type": "string",
                                "description": "Short label, 1-5 words"
                            },
                            "description": {
                                "type": "string",
                                "description": "Optional longer explanation of what this option does"
                            },
                            "requires_input": {
                                "type": "boolean",
                                "default": false,
                                "description": "Set true if the user must provide additional input (e.g. a file path, URL, or new name) when choosing this option. The UI will show a text input below the option and require it to be filled before submit."
                            },
                            "input_placeholder": {
                                "type": "string",
                                "default": "",
                                "description": "Placeholder text for the input field (e.g. '/path/to/file' or 'https://...'). Only used when requires_input=true."
                            }
                        },
                        "required": ["label"]
                    }
                },
                "multi_select": {
                    "type": "boolean",
                    "default": false,
                    "description": "Set true if user can select multiple independent options"
                }
            },
            "required": ["question", "header", "options"]
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }

    async fn execute(&self, _args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        // ask_user 不走 execute — 由 commands.rs 的专门分支处理
        // 如果走到这里,说明调用链出了 bug
        Err(ToolError::Other(
            "ask_user.execute should not be called; handled by commands.rs".to_string(),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 工厂 + 单元测试
// ─────────────────────────────────────────────────────────────────────────────

/// 用当前 AppConfig.allowed_paths 构造全部内置 tool
pub fn builtin_tools(allowed_paths: Vec<String>) -> Vec<std::sync::Arc<dyn Tool>> {
    use std::sync::Arc;
    vec![
        Arc::new(ReadFileTool),
        Arc::new(super::file_tools::ListDirectoryTool),
        Arc::new(super::file_tools::SearchFilesTool),
        Arc::new(CreateFileTool {
            allowed_paths: allowed_paths.clone(),
        }),
        Arc::new(OverwriteFileTool {
            allowed_paths: allowed_paths.clone(),
        }),
        Arc::new(AppendFileTool {
            allowed_paths: allowed_paths.clone(),
        }),
        Arc::new(super::file_tools::EditFileTool { allowed_paths }),
        Arc::new(AskUserTool),
        Arc::new(super::websearch::WebSearchTool),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    // 临时目录路径辅助
    fn tmp_paths(tmp: &TempDir) -> Vec<String> {
        vec![tmp.path().to_string_lossy().to_string()]
    }

    // ── check_write_allowed ──

    #[test]
    fn test_check_write_allowed_empty_means_unrestricted() {
        let p = Path::new("/etc/passwd");
        assert!(check_write_allowed(p, &[]).is_ok());
    }

    #[test]
    fn test_check_write_allowed_prefix_match() {
        let allowed = vec!["/Users/me/projects".to_string()];
        let p = Path::new("/Users/me/projects/subdir/foo.txt");
        assert!(check_write_allowed(p, &allowed).is_ok());
    }

    #[test]
    fn test_check_write_allowed_exact_match() {
        let allowed = vec!["/Users/me/foo.txt".to_string()];
        let p = Path::new("/Users/me/foo.txt");
        assert!(check_write_allowed(p, &allowed).is_ok());
    }

    #[test]
    fn test_check_write_allowed_rejects_similar_prefix() {
        // "/foo-bar" 不能匹配 "/foo"
        let allowed = vec!["/foo".to_string()];
        let p = Path::new("/foo-bar/x.txt");
        assert!(check_write_allowed(p, &allowed).is_err());
    }

    #[test]
    fn test_check_write_allowed_rejects_outside() {
        let allowed = vec!["/Users/me/projects".to_string()];
        let p = Path::new("/etc/passwd");
        assert!(check_write_allowed(p, &allowed).is_err());
    }

    #[test]
    fn test_check_write_allowed_rejects_parent_escape() {
        let allowed = vec!["/Users/me/projects".to_string()];
        let p = Path::new("/Users/me/projects/../../etc/passwd");
        assert!(check_write_allowed(p, &allowed).is_err());
    }

    // ── read_file ──

    #[tokio::test]
    async fn test_read_file_success() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("hello.txt");
        tokio::fs::write(&p, "hello world").await.unwrap();
        let tool = ReadFileTool;
        let out = tool
            .execute(
                json!({ "path": p.to_string_lossy() }),
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(out.content, "hello world");
        assert!(!out.is_error);
    }

    #[tokio::test]
    async fn test_read_file_missing() {
        let tool = ReadFileTool;
        let out = tool
            .execute(
                json!({ "path": "/nonexistent_xyz_9999.txt" }),
                ToolContext::default(),
            )
            .await;
        assert!(matches!(out, Err(ToolError::Io(_))));
    }

    #[tokio::test]
    async fn test_read_file_no_path_arg() {
        let tool = ReadFileTool;
        let out = tool.execute(json!({}), ToolContext::default()).await;
        assert!(matches!(out, Err(ToolError::InvalidArgs(_))));
    }

    // ── create_file ──

    #[tokio::test]
    async fn test_create_file_success() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("new.txt");
        let tool = CreateFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": p.to_string_lossy(), "content": "abc" }),
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(!out.is_error);
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "abc");
    }

    #[tokio::test]
    async fn test_create_file_rejects_existing() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("exists.txt");
        tokio::fs::write(&p, "old").await.unwrap();
        let tool = CreateFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": p.to_string_lossy(), "content": "new" }),
                ToolContext::default(),
            )
            .await;
        assert!(matches!(out, Err(ToolError::InvalidArgs(_))));
        // 原文件未动
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "old");
    }

    #[tokio::test]
    async fn test_create_file_rejects_outside_allowed() {
        let tmp = TempDir::new().unwrap();
        let tool = CreateFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": "/etc/evil.txt", "content": "x" }),
                ToolContext::default(),
            )
            .await;
        assert!(matches!(out, Err(ToolError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_create_file_no_allowed_means_unrestricted() {
        // allowed_paths 为空 → 不限制
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("free.txt");
        let tool = CreateFileTool {
            allowed_paths: vec![],
        };
        let out = tool
            .execute(
                json!({ "path": p.to_string_lossy(), "content": "ok" }),
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(!out.is_error);
    }

    #[tokio::test]
    async fn test_create_file_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a/b/c.txt");
        let tool = CreateFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        tool.execute(
            json!({ "path": p.to_string_lossy(), "content": "x" }),
            ToolContext::default(),
        )
        .await
        .unwrap();
        assert!(p.exists());
    }

    // ── overwrite_file ──

    #[tokio::test]
    async fn test_overwrite_file_success() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("file.txt");
        tokio::fs::write(&p, "old").await.unwrap();
        let tool = OverwriteFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": p.to_string_lossy(), "content": "new" }),
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(!out.is_error);
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "new");
    }

    #[tokio::test]
    async fn test_overwrite_file_rejects_missing() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("nope.txt");
        let tool = OverwriteFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": p.to_string_lossy(), "content": "x" }),
                ToolContext::default(),
            )
            .await;
        assert!(matches!(out, Err(ToolError::InvalidArgs(_))));
    }

    #[tokio::test]
    async fn test_overwrite_file_rejects_outside_allowed() {
        let tmp = TempDir::new().unwrap();
        let tool = OverwriteFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": "/etc/passwd", "content": "x" }),
                ToolContext::default(),
            )
            .await;
        assert!(matches!(out, Err(ToolError::PermissionDenied(_))));
    }

    // ── append_file ──

    #[tokio::test]
    async fn test_append_file_creates_when_missing() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("new.txt");
        let tool = AppendFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": p.to_string_lossy(), "content": "abc" }),
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.content.contains("已创建"));
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "abc");
    }

    #[tokio::test]
    async fn test_append_file_appends_when_exists() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("ex.txt");
        tokio::fs::write(&p, "line1\n").await.unwrap();
        let tool = AppendFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        tool.execute(
            json!({ "path": p.to_string_lossy(), "content": "line2\n" }),
            ToolContext::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&p).await.unwrap(),
            "line1\nline2\n"
        );
    }

    #[tokio::test]
    async fn test_append_file_rejects_outside_allowed() {
        let tmp = TempDir::new().unwrap();
        let tool = AppendFileTool {
            allowed_paths: tmp_paths(&tmp),
        };
        let out = tool
            .execute(
                json!({ "path": "/etc/passwd", "content": "x" }),
                ToolContext::default(),
            )
            .await;
        assert!(matches!(out, Err(ToolError::PermissionDenied(_))));
    }

    // ── builtin_tools 工厂 + 重复名检查 ──

    #[test]
    fn test_builtin_tools_have_unique_names() {
        let tools = builtin_tools(vec![]);
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "tool 名重复: {:?}", names);
    }

    #[test]
    fn test_builtin_safety_classification() {
        let tools = builtin_tools(vec![]);
        for t in &tools {
            match t.name() {
                "read_file" | "list_directory" | "search_files" | "ask_user" | "websearch" => {
                    assert_eq!(t.safety(), ToolSafety::ReadOnly)
                }
                "create_file" | "overwrite_file" | "append_file" | "edit_file" => {
                    assert_eq!(t.safety(), ToolSafety::Write)
                }
                _ => panic!("unexpected tool: {}", t.name()),
            }
        }
    }
}
