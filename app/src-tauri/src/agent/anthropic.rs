//! Anthropic 协议（messages 流式）：Claude 系列模型。

use serde_json::{json, Value};

use super::provider::{Block, Msg, Role, SseBuffer, Sink, Streamed};
use super::settings::AgentSettings;

/// 规范消息 → messages 数组（system 单独走顶层字段；工具结果在 user 消息的 tool_result 块里）
fn translate(msgs: &[Msg]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for m in msgs {
        let mut blocks: Vec<Value> = Vec::new();
        // 工具结果在前，随后才是文字
        for b in &m.blocks {
            if let Block::ToolResult { call_id, text } = b {
                blocks.push(json!({ "type": "tool_result", "tool_use_id": call_id, "content": text }));
            }
        }
        for b in &m.blocks {
            match b {
                Block::Text(t) if !t.is_empty() => blocks.push(json!({ "type": "text", "text": t })),
                Block::ToolUse(c) => {
                    let args: Value = serde_json::from_str(&c.args).unwrap_or(json!({}));
                    blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": args }));
                }
                _ => {}
            }
        }
        if blocks.is_empty() {
            continue;
        }
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        match out.last_mut() {
            // 协议要求相邻消息同角色时合并
            Some(last) if last["role"] == role => {
                last["content"]
                    .as_array_mut()
                    .expect("content array")
                    .extend(blocks);
            }
            _ => out.push(json!({ "role": role, "content": blocks })),
        }
    }
    out
}

fn wire_tools(defs: &[(String, String, Value)]) -> Vec<Value> {
    defs.iter()
        .map(|(name, desc, schema)| {
            json!({ "name": name, "description": desc, "input_schema": schema })
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
    let base = if cfg.base_url.trim().is_empty() {
        "https://api.anthropic.com"
    } else {
        cfg.base_url.trim_end_matches('/')
    };
    let url = format!("{base}/v1/messages");
    let body = json!({
        "model": cfg.model,
        "max_tokens": 8192,
        "system": system,
        "messages": translate(msgs),
        "tools": wire_tools(defs),
        "stream": true,
    });

    let client = super::http_client()?;
    let resp = client
        .post(&url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
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
    // index → (是否工具块, id, name, input json 累积)
    let mut block_kind: std::collections::HashMap<u64, (bool, String, String, String)> =
        Default::default();
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
            let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
            match v.get("type").and_then(|t| t.as_str()) {
                Some("content_block_start") => {
                    let idx = v.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                    if let Some(block) = v.get("content_block") {
                        let kind = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if kind == "tool_use" {
                            let id = block.get("id").and_then(|s| s.as_str()).unwrap_or("");
                            let name = block.get("name").and_then(|s| s.as_str()).unwrap_or("");
                            block_kind.insert(idx, (true, id.to_string(), name.to_string(), String::new()));
                            (sink.on_tool_start)(name, "");
                        } else {
                            block_kind.insert(idx, (false, String::new(), String::new(), String::new()));
                        }
                    }
                }
                Some("content_block_delta") => {
                    let idx = v.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                    if let Some(delta) = v.get("delta") {
                        match delta.get("type").and_then(|t| t.as_str()) {
                            Some("text_delta") => {
                                if let Some(t) = delta.get("text").and_then(|s| s.as_str()) {
                                    (sink.on_text)(t);
                                    out.text.push_str(t);
                                }
                            }
                            Some("input_json_delta") => {
                                if let Some(p) = delta.get("partial_json").and_then(|s| s.as_str()) {
                                    if let Some(entry) = block_kind.get_mut(&idx) {
                                        entry.3.push_str(p);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some("message_start") => {
                    // Anthropic 在 message_start 就给出输入 token 用量
                    if let Some(u) = v.pointer("/message/usage") {
                        let input = u.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0)
                            + u.get("cache_creation_input_tokens").and_then(|x| x.as_u64()).unwrap_or(0)
                            + u.get("cache_read_input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                        let output = u.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                        if input > 0 || output > 0 {
                            out.usage = Some(super::provider::Usage { input, output });
                        }
                    }
                }
                Some("message_delta") => {
                    // stop_reason 在这里到达，工具调用判定交给是否有 tool_use 块；
                    // 同时带来累计输出 token（覆盖 message_start 里的 0）
                    if let Some(u) = v.get("usage") {
                        let output = u.get("output_tokens").and_then(|x| x.as_u64());
                        let input = u.get("input_tokens").and_then(|x| x.as_u64());
                        if let Some(mut usage) = out.usage {
                            if let Some(o) = output {
                                usage.output = o;
                            }
                            if let Some(i) = input {
                                usage.input = i;
                            }
                            out.usage = Some(usage);
                        } else if let Some(o) = output {
                            out.usage = Some(super::provider::Usage { input: input.unwrap_or(0), output: o });
                        }
                    }
                }
                Some("error") => {
                    let msg = v
                        .pointer("/error/message")
                        .and_then(|s| s.as_str())
                        .unwrap_or("模型服务返回错误");
                    return Err(msg.to_string());
                }
                _ => {}
            }
        }
    }

    let mut order: Vec<u64> = block_kind.keys().copied().collect();
    order.sort();
    for idx in order {
        let (is_tool, id, name, raw) = &block_kind[&idx];
        if *is_tool {
            out.tool_calls.push(super::provider::ToolCall {
                id: if id.is_empty() { format!("toolu_{idx}") } else { id.clone() },
                name: name.clone(),
                args: super::provider::parse_args(if raw.is_empty() { "{}" } else { raw }),
            });
        }
    }
    Ok(out)
}

/// 非流式的一次完整回复（压缩上下文用）
pub async fn complete(cfg: &AgentSettings, system: &str, prompt: &str) -> Result<String, String> {
    let base = if cfg.base_url.trim().is_empty() {
        "https://api.anthropic.com"
    } else {
        cfg.base_url.trim_end_matches('/')
    };
    let url = format!("{base}/v1/messages");
    let body = json!({
        "model": cfg.model,
        "max_tokens": 4096,
        "system": system,
        "messages": [{ "role": "user", "content": prompt }],
    });
    let client = super::http_client()?;
    let resp = client
        .post(&url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
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
        .get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
        .trim()
        .to_string();
    if text.is_empty() {
        return Err("模型返回了空摘要，请重试".to_string());
    }
    Ok(text)
}

/// 非流式的最小请求，验证接入配置
pub async fn ping(cfg: &AgentSettings) -> Result<(), String> {
    let base = if cfg.base_url.trim().is_empty() {
        "https://api.anthropic.com"
    } else {
        cfg.base_url.trim_end_matches('/')
    };
    let url = format!("{base}/v1/messages");
    let body = json!({
        "model": cfg.model,
        "max_tokens": 1,
        "messages": [{ "role": "user", "content": "ping" }],
    });
    let client = super::http_client()?;
    let resp = client
        .post(&url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
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
