//! OpenAI 兼容协议（chat/completions 流式）：覆盖 OpenAI、DeepSeek、GLM、Kimi、Qwen、Ollama 等。

use serde_json::{json, Value};

use super::provider::{parse_args, Msg, Role, SseBuffer, Sink, Streamed};
use super::settings::AgentSettings;

/// 规范消息 → chat/completions 消息数组（工具结果必须是紧跟 tool_calls 的 role:"tool" 消息）
fn translate(system: &str, msgs: &[Msg]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in msgs {
        match m.role {
            Role::Assistant => {
                let text = m.text_of();
                let calls: Vec<Value> = m
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        super::provider::Block::ToolUse(c) => Some(json!({
                            "id": c.id, "type": "function",
                            "function": { "name": c.name, "arguments": c.args },
                        })),
                        _ => None,
                    })
                    .collect();
                let mut msg = json!({ "role": "assistant" });
                if !text.is_empty() || calls.is_empty() {
                    msg["content"] = json!(text);
                }
                if !calls.is_empty() {
                    msg["tool_calls"] = json!(calls);
                }
                out.push(msg);
            }
            Role::User => {
                // 工具结果在前（协议要求紧跟 assistant 的 tool_calls），文字在后
                for b in &m.blocks {
                    if let super::provider::Block::ToolResult { call_id, text } = b {
                        out.push(json!({ "role": "tool", "tool_call_id": call_id, "content": text }));
                    }
                }
                let text = m
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        super::provider::Block::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if !text.is_empty() {
                    out.push(json!({ "role": "user", "content": text }));
                }
            }
        }
    }
    out
}

fn wire_tools(defs: &[(String, String, Value)]) -> Vec<Value> {
    defs.iter()
        .map(|(name, desc, schema)| {
            json!({ "type": "function", "function": { "name": name, "description": desc, "parameters": schema } })
        })
        .collect()
}

/// 一次流式调用；cancelled 为 true 时尽快返回 Err
pub async fn stream(
    cfg: &AgentSettings,
    system: &str,
    msgs: &[Msg],
    defs: &[(String, String, Value)],
    sink: &mut Sink<'_>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Streamed, String> {
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "messages": translate(system, msgs),
        "tools": wire_tools(defs),
        "stream": true,
        // 让厂商在流末尾回传 usage（否则流式永远拿不到真实 token 数）
        "stream_options": { "include_usage": true },
    });

    let client = super::http_client()?;
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("连接模型服务失败: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(super::api_error(status.as_str(), &text));
    }

    let mut out = Streamed::default();
    // index → (id, name, arguments 累积)
    let mut pending: std::collections::HashMap<u64, (String, String, String)> = Default::default();
    let mut order: Vec<u64> = Vec::new();
    let mut sse = SseBuffer::new();
    let mut resp = resp;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("读取模型响应失败: {e}"))?
    {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(super::CANCELLED_MSG.to_string());
        }
        for data in sse.push(&chunk) {
            if data == "[DONE]" {
                break;
            }
            let v: Value = match serde_json::from_str(&data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(choice) = v.get("choices").and_then(|c| c.get(0)) else {
                // usage 帧没有 choices（OpenAI 兼容的约定）
                if let Some(u) = v.get("usage") {
                    let input = u.get("prompt_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                    let output = u.get("completion_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                    if input > 0 || output > 0 {
                        out.usage = Some(super::provider::Usage { input, output });
                    }
                }
                continue;
            };
            if let Some(delta) = choice.get("delta") {
                if let Some(t) = delta.get("content").and_then(|v| v.as_str()) {
                    if !t.is_empty() {
                        (sink.on_text)(t);
                        out.text.push_str(t);
                    }
                }
                if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                    for c in calls {
                        let idx = c.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
                        let entry = pending.entry(idx).or_insert_with(|| {
                            order.push(idx);
                            (String::new(), String::new(), String::new())
                        });
                        if let Some(id) = c.get("id").and_then(|v| v.as_str()) {
                            entry.0 = id.to_string();
                        }
                        if let Some(f) = c.get("function") {
                            if let Some(n) = f.get("name").and_then(|v| v.as_str()) {
                                entry.1 = n.to_string();
                            }
                            if let Some(a) = f.get("arguments").and_then(|v| v.as_str()) {
                                entry.2.push_str(a);
                            }
                        }
                    }
                }
            }
        }
    }

    for idx in order {
        let (id, name, args) = pending.remove(&idx).ok_or("工具调用数据不完整")?;
        (sink.on_tool_start)(&name, &args);
        out.tool_calls.push(super::provider::ToolCall {
            id: if id.is_empty() { format!("call_{idx}") } else { id },
            name,
            args: parse_args(&args),
        });
    }
    Ok(out)
}

/// 非流式的一次完整回复（压缩上下文用：只要一段文字，不需要流式与工具）
pub async fn complete(cfg: &AgentSettings, system: &str, prompt: &str) -> Result<String, String> {
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt },
        ],
        "stream": false,
    });
    let client = super::http_client()?;
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("连接模型服务失败: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(super::api_error(status.as_str(), &text));
    }
    let v: Value = resp.json().await.map_err(|e| format!("解析模型响应失败: {e}"))?;
    let text = v
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err("模型返回了空摘要，请重试".to_string());
    }
    Ok(text)
}

/// 非流式的最小请求，验证接入配置
pub async fn ping(cfg: &AgentSettings) -> Result<(), String> {
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "messages": [{ "role": "user", "content": "ping" }],
        "max_tokens": 1,
        "stream": false,
    });
    let client = super::http_client()?;
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("连接失败: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(super::api_error(status.as_str(), &text))
    }
}
