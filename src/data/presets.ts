export interface Preset {
  id: string;
  name: string;
  model: string;
  upstream: string;
  contextWindow: number;
  maxOutputTokens: number;
  label: string;
}

export const BUILTIN_PRESETS: Preset[] = [
  // ── Kimi ──
  {
    id: "preset-kimi-cp",
    name: "Kimi Coding Plan",
    model: "kimi-for-coding",
    upstream: "https://api.kimi.com/coding/v1",
    contextWindow: 262144, maxOutputTokens: 32768,
    label: "Kimi Coding Plan (月之暗面)",
  },
  {
    id: "preset-kimi-api",
    name: "Kimi API",
    model: "kimi-k2.7-code",
    upstream: "https://api.moonshot.cn/v1",
    contextWindow: 262144, maxOutputTokens: 32768,
    label: "Kimi API (月之暗面)",
  },
  // ── GLM (智谱) ──
  {
    id: "preset-glm-cp",
    name: "GLM Coding Plan",
    model: "glm-5.2",
    upstream: "https://open.bigmodel.cn/api/anthropic/v1",
    contextWindow: 200000, maxOutputTokens: 32768,
    label: "GLM Coding Plan (智谱)",
  },
  {
    id: "preset-glm-api",
    name: "GLM API",
    model: "glm-4.7",
    upstream: "https://open.bigmodel.cn/api/paas/v4",
    contextWindow: 128000, maxOutputTokens: 4096,
    label: "GLM API (智谱)",
  },
  // ── DeepSeek ──
  {
    id: "preset-deepseek",
    name: "DeepSeek",
    model: "deepseek-v4-pro",
    upstream: "https://api.deepseek.com/anthropic/v1",
    contextWindow: 1000000, maxOutputTokens: 384000,
    label: "DeepSeek",
  },
  // ── 火山方舟 (豆包) ──
  {
    id: "preset-volc-cp",
    name: "Volcengine AgentPlan",
    model: "doubao-seed-2.0",
    upstream: "https://ark.cn-beijing.volces.com/api/anthropic/v1",
    contextWindow: 200000, maxOutputTokens: 32768,
    label: "火山 AgentPlan (豆包)",
  },
  {
    id: "preset-volc-api",
    name: "Volcengine API",
    model: "doubao-1.5-pro-256k",
    upstream: "https://ark.cn-beijing.volces.com/api/v3",
    contextWindow: 256000, maxOutputTokens: 16384,
    label: "火山 API (豆包)",
  },
  // ── 阿里百炼 (通义) ──
  {
    id: "preset-bailian-cp",
    name: "Bailian Coding Plan",
    model: "qwen-plus",
    upstream: "https://dashscope.aliyuncs.com/compatible-mode/anthropic/v1",
    contextWindow: 200000, maxOutputTokens: 32768,
    label: "百炼 Coding Plan (通义)",
  },
  {
    id: "preset-bailian-api",
    name: "Bailian API",
    model: "qwen-max",
    upstream: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    contextWindow: 32768, maxOutputTokens: 8192,
    label: "百炼 API (通义)",
  },
  // ── 国外 ──
  {
    id: "preset-openai",
    name: "OpenAI GPT-5.5",
    model: "gpt-5.5",
    upstream: "https://api.openai.com/v1",
    contextWindow: 272000, maxOutputTokens: 128000,
    label: "OpenAI (GPT-5.5)",
  },
  {
    id: "preset-anthropic",
    name: "Claude Opus 4",
    model: "claude-opus-4-20250514",
    upstream: "https://api.anthropic.com/v1",
    contextWindow: 200000, maxOutputTokens: 32768,
    label: "Anthropic (Claude Opus 4)",
  },
  {
    id: "preset-google",
    name: "Gemini 2.5 Pro",
    model: "gemini-2.5-pro",
    upstream: "https://generativelanguage.googleapis.com/v1beta",
    contextWindow: 1048576, maxOutputTokens: 65536,
    label: "Google (Gemini 2.5 Pro)",
  },
];

export function findPreset(model: string): Preset | undefined {
  return BUILTIN_PRESETS.find(p => p.model === model);
}
