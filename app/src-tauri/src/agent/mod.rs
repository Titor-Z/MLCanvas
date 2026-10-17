//! 内置 Agent：用户在应用内接入自己的模型，助手在进程内直调画板工具（与外部 MCP 客户端共用
//! ws_server::handle_request 一套执行层，会话契约 / busy 状态 / 活动流归属自动继承）。
//! Rust 侧跑回合循环（流式 + 工具调用），前端是对话 UI；模型配置落盘 ~/YuCanvas/.settings.json。

mod anthropic;
mod openai;
pub mod provider;
mod settings;

use provider::{Block, Msg, Role, Sink, Streamed};
use settings::{AgentSettings, ConfigEntry};

pub use settings::default_parent;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

const CANCELLED_MSG: &str = "已停止";
const MAX_ROUNDS: usize = 25;

static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TURN_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn http_client() -> Result<reqwest::Client, String> {
    static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .read_timeout(std::time::Duration::from_secs(120))
                .build()
                .ok()
        })
        .clone()
        .ok_or_else(|| "初始化网络客户端失败".to_string())
}

/// 把厂商错误正文压成一句人话（两家协议的错误体都是 error.message）
pub(super) fn api_error(status: &str, body: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    let detail = parsed
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let head = match status {
        "401" | "403" => "密钥无效或无权限",
        "404" => "接口地址或模型名称不存在",
        "429" => "请求过于频繁或额度不足",
        _ => "模型服务返回错误",
    };
    if detail.is_empty() {
        format!("{head}（{status}）")
    } else {
        format!("{head}（{status}）：{detail}")
    }
}

// ── 公共模型库（models.dev）──

const MODELS_URL: &str = "https://models.dev/api.json";
const CATALOG_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

fn catalog_file() -> std::path::PathBuf {
    crate::state::projects_root().join(".models.json")
}

/// 取 models.dev 全量 JSON（24h 缓存；网络失败回退旧缓存）
async fn models_json() -> Result<String, String> {
    let file = catalog_file();
    let fresh = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|age| age < CATALOG_TTL)
        .unwrap_or(false);
    if fresh {
        if let Ok(text) = std::fs::read_to_string(&file) {
            return Ok(text);
        }
    }
    let client = http_client()?;
    if let Ok(resp) = client.get(MODELS_URL).send().await {
        if let Ok(text) = resp.text().await {
            let _ = std::fs::create_dir_all(crate::state::projects_root());
            let _ = std::fs::write(&file, &text);
            return Ok(text);
        }
    }
    std::fs::read_to_string(&file).map_err(|_| "模型库暂不可用".to_string())
}

/// 按接口地址匹配 models.dev 里的服务商，返回其模型清单（最近更新在前）。
async fn model_catalog(base_url: &str) -> Result<Vec<Value>, String> {
    let text = models_json().await?;
    let root: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let root = root.as_object().ok_or("模型库格式异常")?;
    let target = base_url.trim().trim_end_matches('/');
    if target.is_empty() {
        return Ok(Vec::new());
    }
    // 最长前缀匹配，避免同族服务商（alibaba / alibaba-cn）互相抢
    let mut best: Option<&Value> = None;
    let mut best_len = 0usize;
    for provider in root.values() {
        let api = provider
            .get("api")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim_end_matches('/');
        if !api.is_empty() && target.starts_with(api) && api.len() > best_len {
            best_len = api.len();
            best = Some(provider);
        }
    }
    let Some(models) = best.and_then(|p| p.get("models")).and_then(|v| v.as_object()) else {
        return Ok(Vec::new());
    };
    let mut list: Vec<(String, Value)> = models
        .iter()
        .map(|(id, m)| {
            let updated = m.get("last_updated").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let context = m.pointer("/limit/context").and_then(|c| c.as_u64()).unwrap_or(0);
            let reasoning = m.get("reasoning").and_then(|b| b.as_bool()).unwrap_or(false);
            let vision = m
                .pointer("/modalities/input")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().any(|x| x.as_str() == Some("image")))
                .unwrap_or(false);
            (
                updated,
                json!({
                    "id": id,
                    "name": m.get("name").and_then(|s| s.as_str()).unwrap_or(id),
                    "context": context,
                    "reasoning": reasoning,
                    "vision": vision,
                }),
            )
        })
        .collect();
    list.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(list.into_iter().map(|(_, v)| v).collect())
}

// ── 回合循环 ──

struct TurnGuard {
    app: AppHandle,
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        crate::ws_server::busy_exit(&self.app);
        TURN_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn system_prompt(app: &AppHandle) -> String {
    let project = app
        .state::<crate::state::AppState>()
        .active_project
        .lock()
        .expect("active_project lock poisoned")
        .clone();
    format!(
        r#"你是「聿Canvas」内置的设计助手。聿Canvas 是 Agent 优先的 UI 设计工具：用户描述想要的界面，你直接在项目画布上画出来。

当前活动项目：{project}。你的全部操作通过工具完成，会作用于这个项目。

工作方式（务必遵守）：
- 接到设计任务先调用 get_conventions 装载《聿Canvas Agent 工作手册》，严格按其中的六步流程工作；项目已有风格规范时用 get_design_spec 读入，全程遵守令牌体系，不要凭空假设风格。
- 画板是静态设计图：纯 HTML/CSS/JS，不写交互逻辑；新画板用语义命名（禁 tmp/临时 等工程名），先 register_artboard 再 write_artboard。
- 每画完一块画板，立即用 screenshot_artboard 自检、再 list_artboards 对账索引；发现问题改完再验，确认无误才继续。
- 一次对话里画板内容会持久保存；历史只保留文字，需要回顾自己做过什么时用 list_artboards / read_artboard。
- screenshot_artboard 目前只能确认截图成功，不能看图内容：自查请结合画板源码与手册清单，也可以用 query_element 查渲染后的元素结构。

回复口吻：简短、笃定、面向动作，用中文；先说结果，再说要点。"#
    )
}

fn file_hint(args: &serde_json::Value) -> Option<String> {
    ["file_name", "new_name", "name"]
        .iter()
        .find_map(|k| args.get(k).and_then(|v| v.as_str()).map(String::from))
}

/// 工具结果 → 喂回模型的文本；截图不喂图（V1 无多模态），改为文字说明
fn result_text(tool: &str, result: &serde_json::Value) -> String {
    if tool == "screenshot_artboard" {
        return json!({
            "file_name": result.get("file_name"),
            "note": "截图已生成（PNG）。当前对话不支持查看图片内容：请结合画板源码与设计手册的自查清单核对效果，或用 query_element 查渲染后的元素结构。",
        })
        .to_string();
    }
    match result {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// 按接口地址 + 模型 id 查 models.dev 里的上下文上限（查不到返回 None，UI 就不显示环）
async fn context_limit(base_url: &str, model: &str) -> Option<u64> {
    let models = model_catalog(base_url).await.ok()?;
    models
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(model))
        .and_then(|m| m.get("context"))
        .and_then(|v| v.as_u64())
        .filter(|n| *n > 0)
}

/// 一个用户回合：流式取模型回复 → 有工具调用就执行（进程内直调）→ 结果喂回 → 循环
async fn run_turn(app: AppHandle, history: Vec<Msg>) -> Result<String, String> {
    let store = settings::load();
    let Some(entry) = store.active() else {
        return Err("尚未接入模型：请先在设置里连接你的模型服务".to_string());
    };
    if !entry.is_configured() {
        return Err("尚未接入模型：请先在设置里连接你的模型服务".to_string());
    }
    let cfg = entry.wire();
    let model_name = if entry.provider.trim().is_empty() {
        entry.model.clone()
    } else {
        format!("{} · {}", entry.provider, entry.model)
    };
    if !app
        .state::<crate::state::AppState>()
        .project_opened
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("尚未打开项目：请先创建或打开一个项目，再与助手对话".to_string());
    }
    if TURN_ACTIVE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("助手正在工作中，请等当前任务完成".to_string());
    }
    let _guard = TurnGuard { app: app.clone() };
    crate::ws_server::busy_enter(&app);
    CANCELLED.store(false, std::sync::atomic::Ordering::SeqCst);

    let defs = provider::tool_defs();
    let system = system_prompt(&app);
    let mut msgs = history;

    // 上下文环的数据源：模型上限来自 models.dev，用量来自厂商流式回执
    let limit = context_limit(&cfg.base_url, &cfg.model).await;
    let _ = app.emit("agent-chat-model", json!({ "name": model_name }));
    let mut usage = (0u64, 0u64);
    let emit_usage = |used_input: u64, used_output: u64| {
        let _ = app.emit(
            "agent-chat-usage",
            json!({
                "input": used_input,
                "output": used_output,
                "used": used_input + used_output,
                "limit": limit,
            }),
        );
    };

    // 文本增量缓冲：攒到 48 字节或 90ms 才 emit；每轮流结束与回合结束前强制 flush
    type DeltaBuf = (String, std::time::Instant);
    let buffer = std::sync::Arc::new(std::sync::Mutex::new(
        (String::new(), std::time::Instant::now()),
    ));
    let flush = |buf: &std::sync::Arc<std::sync::Mutex<DeltaBuf>>| {
        let mut b = buf.lock().expect("delta buffer lock poisoned");
        if !b.0.is_empty() {
            let payload = std::mem::take(&mut b.0);
            let _ = app.emit("agent-chat-delta", json!({ "text": payload }));
        }
        b.1 = std::time::Instant::now();
    };
    let sink_buffer = buffer.clone();
    let mut sink = Sink::new(
        |t: &str| {
            let mut b = sink_buffer.lock().expect("delta buffer lock poisoned");
            b.0.push_str(t);
            if b.0.len() >= 48 || b.1.elapsed() >= std::time::Duration::from_millis(90) {
                let payload = std::mem::take(&mut b.0);
                let _ = app.emit("agent-chat-delta", json!({ "text": payload }));
                b.1 = std::time::Instant::now();
            }
        },
        |_: &str, _: &str| {},
    );

    let mut seq = 0usize;
    for _round in 0..MAX_ROUNDS {
        let streamed: Streamed = match cfg.protocol.as_str() {
            "anthropic" => anthropic::stream(&cfg, &system, &msgs, &defs, &mut sink, &CANCELLED).await?,
            _ => openai::stream(&cfg, &system, &msgs, &defs, &mut sink, &CANCELLED).await?,
        };
        flush(&buffer);

        if let Some(u) = streamed.usage {
            usage = (u.input, usage.1 + u.output);
            emit_usage(usage.0, usage.1);
        }

        if streamed.tool_calls.is_empty() {
            let text = streamed.text.trim().to_string();
            if text.is_empty() {
                return Err("模型返回了空回复，请重试".to_string());
            }
            return Ok(text);
        }

        // 助手消息（文字 + 工具调用）入历史
        let mut blocks: Vec<Block> = Vec::new();
        if !streamed.text.is_empty() {
            blocks.push(Block::Text(streamed.text.clone()));
        }
        for c in &streamed.tool_calls {
            blocks.push(Block::ToolUse(c.clone()));
        }
        msgs.push(Msg { role: Role::Assistant, blocks });

        let mut results: Vec<Block> = Vec::new();
        for call in &streamed.tool_calls {
            if CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(CANCELLED_MSG.to_string());
            }
            let args: serde_json::Value = serde_json::from_str(&call.args).unwrap_or(json!({}));
            let _ = app.emit(
                "agent-chat-tool",
                json!({ "tool": call.name, "file": file_hint(&args) }),
            );

            seq += 1;
            let req = json!({ "id": seq, "tool": call.name, "args": args });
            let work_app = app.clone();
            let resp = tauri::async_runtime::spawn_blocking(move || {
                crate::ws_server::handle_request(req, &work_app)
            })
            .await
            .map_err(|e| format!("工具执行调度失败: {e}"))?;

            let text = if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                result_text(&call.name, resp.get("result").unwrap_or(&json!(null)))
            } else {
                format!(
                    "工具执行出错：{}",
                    resp.get("error").and_then(|v| v.as_str()).unwrap_or("未知错误")
                )
            };
            results.push(Block::ToolResult { call_id: call.id.clone(), text });
        }
        msgs.push(Msg { role: Role::User, blocks: results });
    }
    Err(format!("工具调用超过 {MAX_ROUNDS} 轮仍未完成，已停止。请把任务拆小后再试"))
}

// ── Tauri 命令 ──

#[tauri::command]
pub fn agent_get_settings() -> serde_json::Value {
    let s = settings::load();
    json!({
        "current": s.current,
        "configs": s.configs,
        "default_parent": s.default_parent,
        "configured": s.is_configured(),
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// 新增或更新一份模型配置；返回配置 id。新增且此前无当前配置时自动设为当前。
#[tauri::command]
pub fn agent_save_config(
    id: Option<String>,
    provider: String,
    protocol: String,
    base_url: String,
    api_key: String,
    model: String,
) -> Result<String, String> {
    let mut store = settings::load();
    let id = id.unwrap_or_default();
    let candidate = ConfigEntry {
        id: id.clone(),
        provider: provider.trim().to_string(),
        protocol,
        base_url: base_url.trim().to_string(),
        api_key: api_key.trim().to_string(),
        model: model.trim().to_string(),
    };
    candidate.validate()?;

    let saved_id = if id.is_empty() {
        let mut entry = candidate;
        entry.id = settings::new_config_id(&entry.provider);
        if store.current.is_empty() {
            store.current = entry.id.clone();
        }
        let new_id = entry.id.clone();
        store.configs.push(entry);
        new_id
    } else {
        let slot = store
            .configs
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| "配置不存在".to_string())?;
        let version = candidate.clone();
        *slot = ConfigEntry { id: id.clone(), ..version };
        id
    };
    settings::save(&store)?;
    Ok(saved_id)
}

#[tauri::command]
pub fn agent_delete_config(id: String) -> Result<(), String> {
    let mut store = settings::load();
    store.configs.retain(|c| c.id != id);
    if store.current == id {
        store.current = store.configs.first().map(|c| c.id.clone()).unwrap_or_default();
    }
    settings::save(&store)
}

#[tauri::command]
pub fn agent_set_current(id: String) -> Result<(), String> {
    let mut store = settings::load();
    if !store.configs.iter().any(|c| c.id == id) {
        return Err("配置不存在".to_string());
    }
    store.current = id;
    settings::save(&store)
}

/// 新建项目默认父目录：空串 = 恢复默认（~/YuCanvas）。
#[tauri::command]
pub fn agent_set_default_parent(dir: String) -> Result<(), String> {
    let mut store = settings::load();
    store.default_parent = dir.trim().to_string();
    settings::save(&store)
}

/// 公共模型库（models.dev）：按接口地址匹配服务商，返回其模型清单。
/// 结果缓存 24h；网络失败回退过期缓存，无缓存则返回空清单。
#[tauri::command]
pub async fn agent_model_catalog(base_url: String) -> serde_json::Value {
    match model_catalog(&base_url).await {
        Ok(models) => json!({ "models": models, "source": "models.dev" }),
        Err(_) => json!({ "models": [], "source": "" }),
    }
}

#[tauri::command]
pub async fn agent_test_connection(
    protocol: String,
    base_url: String,
    api_key: String,
    model: String,
) -> Result<(), String> {
    let cfg = AgentSettings { protocol, base_url, api_key, model };
    cfg.validate()?;
    match cfg.protocol.as_str() {
        "anthropic" => anthropic::ping(&cfg).await,
        _ => openai::ping(&cfg).await,
    }
}

#[tauri::command]
pub async fn agent_chat_turn(
    app: AppHandle,
    messages: Vec<ChatTurnMsg>,
) -> Result<String, String> {
    let history: Vec<Msg> = messages
        .into_iter()
        .map(|m| match m.role.as_str() {
            "assistant" => Msg::assistant(m.content),
            _ => Msg::user(m.content),
        })
        .collect();
    run_turn(app, history).await
}

#[tauri::command]
pub async fn agent_chat_cancel() {
    CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// 压缩上下文：把给定的一段历史整理成工作纪要（不跑工具、不动画板）
#[tauri::command]
pub async fn agent_chat_compact(
    messages: Vec<ChatTurnMsg>,
) -> Result<serde_json::Value, String> {
    let store = settings::load();
    let Some(entry) = store.active() else {
        return Err("尚未接入模型：请先在设置里连接你的模型服务".to_string());
    };
    if !entry.is_configured() {
        return Err("尚未接入模型：请先在设置里连接你的模型服务".to_string());
    }
    let count = messages.len();
    let transcript = messages
        .iter()
        .map(|m| {
            let who = if m.role == "assistant" { "助手" } else { "用户" };
            format!("{who}：{}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let system = "你是设计工作记录的整理者。把对话整理成一份简短的工作纪要，供后续对话接着做。";
    let prompt = format!(
        r#"把下面这段设计对话整理成工作纪要，用中文，不超过 400 字。
必须保留：用户的目标与偏好、已经画了哪些画板（名称与用途）、当前采用的风格与令牌约定、还没做完的事。
不要保留：寒暄、重复表述、工具调用的中间过程。
直接输出纪要正文，不要前言。

---
{transcript}
---"#
    );

    let cfg = entry.wire();
    let summary = match cfg.protocol.as_str() {
        "anthropic" => anthropic::complete(&cfg, system, &prompt).await?,
        _ => openai::complete(&cfg, system, &prompt).await?,
    };
    Ok(json!({ "summary": summary, "count": count }))
}

#[derive(serde::Deserialize)]
pub struct ChatTurnMsg {
    pub role: String,
    pub content: String,
}
