//! 内置 Agent 的消息规范层与 SSE 公共件。
//! 对话历史用统一的消息块模型（贴近 Anthropic 的 blocks 设计），两个协议适配器各自翻译成自家 wire 格式。

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// 一次工具调用（args 保存 JSON 对象原文，供模型侧往返）
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

#[derive(Debug, Clone)]
pub enum Block {
    Text(String),
    ToolUse(ToolCall),
    ToolResult { call_id: String, text: String },
}

#[derive(Debug, Clone)]
pub struct Msg {
    pub role: Role,
    pub blocks: Vec<Block>,
}

impl Msg {
    pub fn user(text: impl Into<String>) -> Self {
        Msg { role: Role::User, blocks: vec![Block::Text(text.into())] }
    }
    pub fn assistant(text: impl Into<String>) -> Self {
        Msg { role: Role::Assistant, blocks: vec![Block::Text(text.into())] }
    }
    /// 纯文本消息的文本（用于构建只含文字的历史）
    pub fn text_of(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// 一轮流式调用的产物：正文文本 + 需要执行的工具调用 + 用量
#[derive(Debug, Default)]
pub struct Streamed {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    /// 本轮流式响应报告的实际用量（厂商未报告时保持 None）
    pub usage: Option<Usage>,
}

/// 一次请求的 token 用量（两家协议的字段归一到这个形状）
#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    /// 输入 token（含缓存命中部分；Anthropic 的三项已合并）
    pub input: u64,
    pub output: u64,
}

/// 流式回调：文本增量与工具调用开始（供 UI 实时反馈）
pub struct Sink<'a> {
    pub on_text: Box<dyn FnMut(&str) + Send + 'a>,
    pub on_tool_start: Box<dyn FnMut(&str, &str) + Send + 'a>,
}

impl<'a> Sink<'a> {
    pub fn new(
        on_text: impl FnMut(&str) + Send + 'a,
        on_tool_start: impl FnMut(&str, &str) + Send + 'a,
    ) -> Self {
        Sink { on_text: Box::new(on_text), on_tool_start: Box::new(on_tool_start) }
    }
}

/// mcp_tools.json → 各协议的工具定义（内置助手与 MCP 共用一份工具面）
pub fn tool_defs() -> Vec<(String, String, Value)> {
    let raw = include_str!("../mcp_tools.json");
    let parsed: Vec<Value> = serde_json::from_str(raw).unwrap_or_default();
    parsed
        .into_iter()
        .filter_map(|t| {
            let name = t.get("name")?.as_str()?.to_string();
            let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let schema = t.get("inputSchema").cloned().unwrap_or(Value::Null);
            Some((name, desc, schema))
        })
        .collect()
}

/// 跨 chunk 的 SSE 事件缓冲：按空行切事件，抽出 data: 载荷
pub struct SseBuffer {
    buf: Vec<u8>,
}

impl SseBuffer {
    pub fn new() -> Self {
        SseBuffer { buf: Vec::new() }
    }

    /// 喂入一段字节，返回其中凑齐的 data 载荷（按序）
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            let Some(pos) = find(&self.buf, b"\n\n") else { break };
            let event: Vec<u8> = self.buf.drain(..pos + 2).collect();
            let text = String::from_utf8_lossy(&event);
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    out.push(data.trim_start().to_string());
                }
            }
        }
        out
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 从流式累积的 tool arguments 文本解析参数对象；解析失败给空对象（结果层会把错误反馈给模型侧）
pub fn parse_args(raw: &str) -> String {
    let v: Value = serde_json::from_str(raw).unwrap_or(Value::Object(serde_json::Map::new()));
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_buffer_across_chunks() {
        let mut b = SseBuffer::new();
        assert!(b.push(b"data: {\"a\"").is_empty());
        let got = b.push(b":1}\n\ndata: [DONE]\n\n");
        assert_eq!(got, vec!["{\"a\":1}", "[DONE]"]);
    }

    #[test]
    fn tool_defs_from_mcp_tools() {
        let defs = tool_defs();
        assert!(defs.len() >= 18);
        assert!(defs.iter().any(|(n, _, _)| n == "write_artboard"));
    }
}
