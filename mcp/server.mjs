#!/usr/bin/env node
// MLCanvas MCP 服务器（stdio）。
// 把 MCP 工具调用转发到 MLCanvas 应用的本地 TCP 桥（127.0.0.1:9915, NDJSON）。
// Agent 接入本 MCP 后即可读写画板、截屏回读渲染结果。

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import net from "node:net";

const BRIDGE_HOST = "127.0.0.1";
const BRIDGE_PORT = 9915;

/** 调用 MLCanvas 应用的 TCP 桥 */
function bridgeCall(tool, args = {}) {
  return new Promise((resolve, reject) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const socket = net.connect(BRIDGE_PORT, BRIDGE_HOST);
    socket.setEncoding("utf8");
    let buf = "";
    const fail = (message) => {
      socket.destroy();
      reject(new Error(message));
    };
    socket.on("connect", () => {
      socket.write(JSON.stringify({ id, tool, args }) + "\n");
    });
    socket.on("data", (chunk) => {
      buf += chunk;
      const nl = buf.indexOf("\n");
      if (nl === -1) return;
      let msg;
      try {
        msg = JSON.parse(buf.slice(0, nl));
      } catch (e) {
        fail(`桥响应解析失败: ${e.message}`);
        return;
      }
      socket.end();
      if (msg.id !== id) return fail("桥响应 id 不匹配");
      if (msg.ok) resolve(msg.result);
      else reject(new Error(msg.error || "未知桥错误"));
    });
    socket.on("error", (e) => {
      fail(
        `无法连接 MLCanvas 应用（${BRIDGE_HOST}:${BRIDGE_PORT}，${e.code}）。请先启动 MLCanvas。`,
      );
    });
  });
}

const text = (t) => ({ type: "text", text: t });

const TOOLS = [
  {
    name: "list_artboards",
    description: "列出 MLCanvas 中所有画板（artboards/ 下的 .html 文件）",
    inputSchema: { type: "object", properties: {} },
  },
  {
    name: "read_artboard",
    description: "读取指定画板的完整 HTML 源码",
    inputSchema: {
      type: "object",
      properties: { file_name: { type: "string", description: "画板文件名，如 hello.html" } },
      required: ["file_name"],
    },
  },
  {
    name: "write_artboard",
    description:
      "写入画板源码（纯 HTML/CSS/JS）。写入后 MLCanvas 会自动刷新对应画板。改完应调用 screenshot_artboard 回读确认效果。",
    inputSchema: {
      type: "object",
      properties: {
        file_name: { type: "string", description: "画板文件名" },
        content: { type: "string", description: "完整 HTML 内容" },
      },
      required: ["file_name", "content"],
    },
  },
  {
    name: "register_artboard",
    description: "注册（创建）一个新画板，MLCanvas 会立刻显示它",
    inputSchema: {
      type: "object",
      properties: {
        file_name: { type: "string", description: "新画板文件名，必须以 .html 结尾" },
        title: { type: "string", description: "可选，画板标题" },
      },
      required: ["file_name"],
    },
  },
  {
    name: "notify_ready",
    description: "完成一步修改后主动通知 MLCanvas 刷新对应画板（兜底机制是文件监听）",
    inputSchema: {
      type: "object",
      properties: { file_name: { type: "string", description: "画板文件名" } },
      required: ["file_name"],
    },
  },
  {
    name: "screenshot_artboard",
    description:
      "截取画板当前渲染结果为 PNG 并返回图像。用于回读渲染效果、检查自己的修改是否符合预期。",
    inputSchema: {
      type: "object",
      properties: { file_name: { type: "string", description: "画板文件名" } },
      required: ["file_name"],
    },
  },
  {
    name: "query_element",
    description:
      "查询画板渲染结果中 (x, y) 坐标处的元素信息（标签、类名、文本、位置），坐标系为画板自身视口，单位 CSS 像素",
    inputSchema: {
      type: "object",
      properties: {
        file_name: { type: "string", description: "画板文件名" },
        x: { type: "number" },
        y: { type: "number" },
      },
      required: ["file_name", "x", "y"],
    },
  },
];

const server = new Server(
  { name: "mlcanvas", version: "0.1.0" },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: TOOLS }));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;
  try {
    switch (name) {
      case "list_artboards": {
        const boards = await bridgeCall(name);
        if (boards.length === 0) return { content: [text("artboards/ 目录为空")] };
        return {
          content: [
            text(boards.map((b) => `${b.file_name} — ${b.path}`).join("\n")),
          ],
        };
      }
      case "read_artboard": {
        const content = await bridgeCall(name, args);
        return { content: [text(content)] };
      }
      case "write_artboard": {
        const path = await bridgeCall(name, args);
        return { content: [text(`已写入 ${path}，画板将自动刷新`)] };
      }
      case "register_artboard": {
        const path = await bridgeCall(name, args);
        return { content: [text(`已创建画板 ${path}`)] };
      }
      case "notify_ready": {
        await bridgeCall(name, args);
        return { content: [text(`已通知刷新 ${args.file_name}`)] };
      }
      case "screenshot_artboard": {
        const shot = await bridgeCall(name, args);
        return {
          content: [
            text(`${shot.file_name} 渲染截图（${shot.mime}）`),
            { type: "image", data: shot.data_base64, mimeType: shot.mime },
          ],
        };
      }
      case "query_element": {
        const res = await bridgeCall(name, args);
        return { content: [text(JSON.stringify(res.element, null, 2))] };
      }
      default:
        return {
          content: [text(`未知工具: ${name}`)],
          isError: true,
        };
    }
  } catch (e) {
    return { content: [text(e.message)], isError: true };
  }
});

const transport = new StdioServerTransport();
await server.connect(transport);
