#!/usr/bin/env node
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ── Config ──────────────────────────────────────────────────────
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PORT = Number(process.env.PROXY_PORT || 15731);
const BIND = process.env.PROXY_BIND || "127.0.0.1";

// Load provider configs from env vars
// Format: CODING_PLAN_PROVIDERS=json
// Or from config file: ~/.coding-plan-proxy.json or ./config.json
function loadProviders() {
  // 1. Env var override
  if (process.env.CODING_PLAN_PROVIDERS) {
    try { return JSON.parse(process.env.CODING_PLAN_PROVIDERS); } catch (e) {
      console.error("Failed to parse CODING_PLAN_PROVIDERS:", e.message);
    }
  }

  // 2. Config file (user home or local)
  const paths = [
    path.join(process.env.HOME || process.env.USERPROFILE || ".", ".coding-plan-proxy.json"),
    path.join(__dirname, "config.json"),
  ];
  for (const p of paths) {
    if (fs.existsSync(p)) {
      try { return JSON.parse(fs.readFileSync(p, "utf8")); } catch (e) {
        console.error(`Failed to load ${p}:`, e.message);
      }
    }
  }

  // 3. Env vars per provider
  const envProviders = {};
  for (const [key, val] of Object.entries(process.env)) {
    const match = key.match(/^CODING_PLAN_(\w+)$/);
    if (match && val) {
      const [model, upstream, apiKey] = val.split("|");
      if (model && upstream) {
        envProviders[model] = { upstream, apiKey: apiKey || process.env[`${match[1]}_API_KEY`] || "" };
      }
    }
  }
  if (Object.keys(envProviders).length > 0) return envProviders;

  // 4. Fallback: empty — proxy will return errors per-request until configured
  console.error("[coding-plan-proxy] No providers configured. Use the desktop app or set CODING_PLAN_PROVIDERS env var.");
  return {};
}

const PROVIDERS = loadProviders();

// ── Helpers ─────────────────────────────────────────────────────
function json(res, status, body) {
  res.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  res.end(JSON.stringify(body));
}

function log(level, msg) {
  const ts = new Date().toISOString().split("T")[1].slice(0, 12);
  console.error(`[${ts}] [${level}] ${msg}`);
}

async function readBody(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}

// ── Responses → Chat Completions ─────────────────────────────
function responsesToChat(body) {
  const messages = [];
  if (body.instructions) messages.push({ role: "system", content: String(body.instructions) });
  const input = body.input;
  if (typeof input === "string") { messages.push({ role: "user", content: input }); return { model: body.model, messages, max_completion_tokens: body.max_output_tokens || 8192 }; }
  if (Array.isArray(input)) {
    for (const item of input) {
      if (item.type === "function_call_output") { messages.push({ role: "tool", tool_call_id: item.call_id, content: String(item.output ?? "") }); continue; }
      if (item.type === "function_call") { let args = {}; try { args = JSON.parse(item.arguments || "{}"); } catch(e) {} messages.push({ role: "assistant", tool_calls: [{ id: item.call_id, type: "function", function: { name: item.name, arguments: JSON.stringify(args) } }] }); continue; }
      let role = item.role || "user"; if (role === "developer") role = "system";
      let content = item.content ?? item.text ?? "";
      if (Array.isArray(content)) content = content.filter(c => c.type === "input_text" || c.type === "output_text").map(c => c.text || "").join("\n");
      if (content) messages.push({ role, content: String(content) });
    }
  }
  if (!messages.length) messages.push({ role: "user", content: "ping" });
  return { model: body.model, messages, max_completion_tokens: body.max_output_tokens || 8192 };
}

// ── Responses → Anthropic Messages ──────────────────────────────
function responsesToAnthropic(body) {
  const messages = [];
  let system = null;
  if (body.instructions) system = String(body.instructions);

  const input = body.input;
  if (typeof input === "string") {
    messages.push({ role: "user", content: input });
  } else if (Array.isArray(input)) {
    for (const item of input) {
      if (item.type === "function_call_output") {
        messages.push({
          role: "user",
          content: [{ type: "tool_result", tool_use_id: item.call_id, content: String(item.output ?? "") }]
        });
        continue;
      }
      if (item.type === "function_call") {
        let args = {};
        try { args = JSON.parse(item.arguments || "{}"); } catch (e) { /* pass */ }
        messages.push({
          role: "assistant",
          content: [{ type: "tool_use", id: item.call_id, name: item.name, input: args }]
        });
        continue;
      }
      let role = item.role || "user";
      if (role === "developer") role = "user";
      let content = item.content ?? item.text ?? "";
      if (Array.isArray(content)) {
        const parts = [];
        for (const part of content) {
          if (part.type === "input_text" || part.type === "output_text") {
            parts.push({ type: "text", text: part.text || "" });
          } else if (part.type === "input_image") {
            parts.push({
              type: "image",
              source: {
                type: "base64",
                media_type: "image/png",
                data: (part.image_url || "").replace(/^data:image\/\w+;base64,/, "")
              }
            });
          }
        }
        content = parts.length ? parts : "";
      }
      if (content) messages.push({ role, content });
    }
  }
  if (!messages.length) messages.push({ role: "user", content: "ping" });

  // Merge consecutive same-role messages
  const cleaned = [];
  for (const msg of messages) {
    if (cleaned.length > 0 && cleaned[cleaned.length - 1].role === msg.role) {
      const prev = cleaned[cleaned.length - 1];
      if (typeof prev.content === "string" && typeof msg.content === "string") {
        prev.content += "\n" + msg.content;
      } else if (Array.isArray(prev.content) && Array.isArray(msg.content)) {
        prev.content = [...prev.content, ...msg.content];
      } else { cleaned.push(msg); }
    } else { cleaned.push(msg); }
  }
  return { messages: cleaned, system };
}

// ── SSE helpers ──────────────────────────────────────────────────
function sse(res, event) {
  res.write(`event: ${event.type}\n`);
  res.write(`data: ${JSON.stringify(event)}\n\n`);
}

// ── Request handler ─────────────────────────────────────────────
async function handleResponses(req, res) {
  const raw = await readBody(req);
  let body;
  try { body = JSON.parse(raw); } catch (e) {
    return json(res, 400, { error: { message: "Invalid JSON" } });
  }

  const model = body.model;
  if (!model) return json(res, 400, { error: { message: "Missing model" } });

  const provider = PROVIDERS[model];
  if (!provider) {
    const known = Object.keys(PROVIDERS).join(", ");
    return json(res, 400, { error: { message: `Unknown model: ${model}. Available: ${known}` } });
  }
  if (!provider.apiKey) return json(res, 401, { error: { message: `No API key for ${model}` } });

  const responseId = `resp_${Date.now().toString(36)}`;
  const isChat = provider.protocol === "chat";
  const endpoint = isChat ? `${provider.upstream.replace(/\/$/, "")}/chat/completions` : `${provider.upstream.replace(/\/$/, "")}/messages`;
  const method = "POST";
  const stream = body.stream !== false;
  
  let upstreamBody;
  const headers = { "content-type": "application/json" };
  if (isChat) {
    headers["Authorization"] = `Bearer ${provider.apiKey}`;
    upstreamBody = responsesToChat(body);
    if (stream) upstreamBody.stream = true;
  } else {
    headers["x-api-key"] = provider.apiKey;
    headers["anthropic-version"] = "2023-06-01";
    const { messages, system } = responsesToAnthropic(body);
    upstreamBody = { model, max_tokens: body.max_output_tokens || 8192, messages, stream };
    if (system) upstreamBody.system = system;
    if (body.temperature != null) upstreamBody.temperature = body.temperature;
  }

  log("info", `→ ${model}  ${endpoint}`);

  let upstreamRes;
  try {
    upstreamRes = await fetch(endpoint, { method, headers, body: JSON.stringify(upstreamBody) });
  } catch (e) {
    log("error", `Upstream error: ${e.message}`);
    return json(res, 502, { error: { message: `Upstream unreachable: ${e.message}` } });
  }

  if (!upstreamRes.ok) {
    const text = await upstreamRes.text();
    log("error", `Upstream ${upstreamRes.status}: ${text.slice(0, 200)}`);
    return json(res, upstreamRes.status, { error: { message: text } });
  }

  // Non-streaming
  if (!stream) {
    const data = await upstreamRes.json();
    const text = isChat
      ? (data.choices?.[0]?.message?.content || "")
      : (data.content?.filter(c => c.type === "text").map(c => c.text).join("") || "");
    return json(res, 200, {
      id: responseId, object: "response",
      created_at: Math.floor(Date.now() / 1000),
      status: "completed", model,
      output: [{
        id: `msg_${Date.now().toString(36)}`, type: "message", role: "assistant",
        content: [{ type: "output_text", text }]
      }],
      output_text: text,
      usage: data.usage ? {
        input_tokens: data.usage.input_tokens || 0,
        output_tokens: data.usage.output_tokens || 0,
        total_tokens: (data.usage.input_tokens || 0) + (data.usage.output_tokens || 0),
      } : undefined,
    });
  }

  // Streaming: SSE → Responses SSE
  res.writeHead(200, {
    "content-type": "text/event-stream; charset=utf-8",
    "cache-control": "no-cache",
    connection: "keep-alive",
  });

  const outputId = `msg_${Date.now().toString(36)}`;
  let outputIndex = 0, textStarted = false, fullText = "";

  sse(res, {
    type: "response.created",
    response: { id: responseId, type: "response", status: "in_progress", model }
  });

  try {
    const reader = upstreamRes.body.getReader();
    const decoder = new TextDecoder();
    let buf = "";

    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      const lines = buf.split(/\r?\n/);
      buf = lines.pop() || "";
      for (const line of lines) {
        if (!line.startsWith("data:")) continue;
        const payload = line.slice(5).trim();
        if (!payload || payload === "[DONE]") continue;

        if (isChat) {
          // Chat Completions SSE → Responses SSE
          let event;
          try { event = JSON.parse(payload); } catch(e) { continue; }
          const delta = event.choices?.[0]?.delta;
          const content = delta?.content || "";
          if (!content) continue;
          if (!textStarted) {
            textStarted = true;
            sse(res, { type: "response.output_item.added", output_index: outputIndex, item: { id: outputId, type: "message", role: "assistant", content: [] } });
            sse(res, { type: "response.content_part.added", item_id: outputId, output_index: outputIndex, content_index: 0, part: { type: "output_text", text: "" } });
          }
          fullText += content;
          sse(res, { type: "response.output_text.delta", item_id: outputId, output_index: outputIndex, content_index: 0, delta: content });
        } else {
          // Anthropic SSE → Responses SSE (existing)
          let event;
          try { event = JSON.parse(payload); } catch(e) { continue; }
          switch (event.type) {
            case "message_start":
              sse(res, { type: "response.output_item.added", output_index: outputIndex, item: { id: outputId, type: "message", role: "assistant", content: [] } });
              break;
            case "content_block_start":
              if (event.content_block?.type === "text" && !textStarted) {
                textStarted = true;
                sse(res, { type: "response.content_part.added", item_id: outputId, output_index: outputIndex, content_index: 0, part: { type: "output_text", text: "" } });
              }
              break;
            case "content_block_delta":
              if (event.delta?.type === "text_delta" && event.delta.text) {
                fullText += event.delta.text;
                sse(res, { type: "response.output_text.delta", item_id: outputId, output_index: outputIndex, content_index: 0, delta: event.delta.text });
              }
              break;
          }
        }
      }
    }
  } catch (e) {
    log("error", `Stream error: ${e.message}`);
  }

  if (textStarted) {
    sse(res, {
      type: "response.output_text.done", item_id: outputId, output_index: outputIndex,
      content_index: 0, text: fullText
    });
    sse(res, {
      type: "response.content_part.done", item_id: outputId, output_index: outputIndex,
      content_index: 0, part: { type: "output_text", text: fullText }
    });
  }
  sse(res, {
    type: "response.output_item.done", output_index: outputIndex,
    item: { id: outputId, type: "message", role: "assistant", content: [{ type: "output_text", text: fullText }] }
  });
  sse(res, {
    type: "response.completed",
    response: { id: responseId, type: "response", status: "completed", model, output_text: fullText }
  });
  res.write("data: [DONE]\n\n");
  res.end();
}

// ── Server ───────────────────────────────────────────────────────
const server = http.createServer((req, res) => {
  if (req.method === "GET" && (req.url === "/health" || req.url === "/")) {
    return json(res, 200, {
      ok: true,
      service: "coding-plan-proxy",
      version: "1.0.0",
      providers: Object.keys(PROVIDERS),
      docs: "https://github.com/NeoMei/coding-plan-proxy"
    });
  }
  if (req.method === "POST" && (req.url === "/responses" || req.url === "/v1/responses")) {
    handleResponses(req, res).catch(e => {
      log("error", `Handler crash: ${e.message}`);
      json(res, 500, { error: { message: "Internal error" } });
    });
    return;
  }
  json(res, 404, { error: { message: "Not found. Use POST /v1/responses" } });
});

server.listen(PORT, BIND, () => {
  log("info", `Coding Plan Proxy v1.0.0  →  http://${BIND}:${PORT}`);
  log("info", `Providers: ${Object.keys(PROVIDERS).join(", ")}`);
});

// Graceful shutdown
process.on("SIGINT", () => { log("info", "Shutting down"); process.exit(0); });
process.on("SIGTERM", () => { log("info", "Shutting down"); process.exit(0); });
