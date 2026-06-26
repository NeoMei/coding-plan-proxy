const zh: Record<string, string> = {
  "app.title": "CodexProxy",
  "proxy.running": "运行中",
  "proxy.stopped": "已停止",
  "proxy.start": "启动代理",
  "proxy.stop": "停止代理",
  "providers.title": "供应商",
  "providers.add": "+ 添加供应商",
  "providers.empty": "暂无供应商，点击「+ 添加供应商」开始",
  "providers.test": "测试连接",
  "providers.apply": "应用到 Codex",
  "providers.edit": "编辑",
  "providers.delete": "删除",
  "providers.keyMissing": "未填 Key",
  "providers.keySet": "已配置",
  "providers.testing": "测试中…",
  "providers.testOk": "连接成功",
  "providers.testFail": "连接失败",
  "providers.testFirst": "请先测试连接",
  "providers.verified": "连接成功",
  "providers.fetchModels": "拉取模型",
  "providers.fetching": "获取中…",
  "providers.foundModels": "找到 {n} 个模型",
  "providers.noModels": "该接口不支持模型列表，请手动输入",
  "providers.enterKeyFirst": "请先填写 API Key",
  "settings.title": "设置",
  "settings.theme": "主题",
  "settings.language": "语言",
  "settings.autoStart": "开机自启",
  "settings.autoStartDesc": "启动应用时自动运行代理",
  "settings.codexConfig": "Codex 配置预览",
  "modal.save": "保存",
  "modal.cancel": "取消",
  "modal.editTitle": "编辑供应商",
  "modal.addTitle": "添加供应商",
  "modal.quickSetup": "快速配置",
  "modal.apiKeyOnly": "仅需填写 API Key",
  "modal.fillRequired": "请填写必填字段",
  "field.name": "显示名称",
  "field.model": "Model Slug",
  "field.upstream": "Upstream URL",
  "field.apiKey": "API Key",
  "field.contextWindow": "上下文窗口",
  "field.maxTokens": "最大输出 Token",
  "preset.select": "选择预设",
  "preset.custom": "自定义",
};

const en: Record<string, string> = {
  "app.title": "CodexProxy",
  "proxy.running": "Running",
  "proxy.stopped": "Stopped",
  "proxy.start": "Start Proxy",
  "proxy.stop": "Stop Proxy",
  "providers.title": "Providers",
  "providers.add": "+ Add Provider",
  "providers.empty": "No providers yet. Click + to add one.",
  "providers.test": "Test",
  "providers.apply": "Apply to Codex",
  "providers.edit": "Edit",
  "providers.delete": "Delete",
  "providers.keyMissing": "Key missing",
  "providers.keySet": "Key set",
  "providers.testing": "Testing…",
  "providers.testOk": "Connected",
  "providers.testFail": "Failed",
  "providers.testFirst": "Test connection first",
  "providers.verified": "Verified",
  "providers.fetchModels": "Fetch Models",
  "providers.fetching": "Fetching…",
  "providers.foundModels": "Found {n} models",
  "providers.noModels": "Endpoint doesn't support model listing",
  "providers.enterKeyFirst": "Enter API key first",
  "settings.title": "Settings",
  "settings.theme": "Theme",
  "settings.language": "Language",
  "settings.autoStart": "Auto-start proxy",
  "settings.autoStartDesc": "Launch proxy on app startup",
  "settings.codexConfig": "Codex Config Preview",
  "modal.save": "Save",
  "modal.cancel": "Cancel",
  "modal.editTitle": "Edit Provider",
  "modal.addTitle": "Add Provider",
  "modal.quickSetup": "Quick Setup",
  "modal.apiKeyOnly": "Only API Key needed",
  "modal.fillRequired": "Fill required fields",
  "field.name": "Display Name",
  "field.model": "Model Slug",
  "field.upstream": "Upstream URL",
  "field.apiKey": "API Key",
  "field.contextWindow": "Context Window",
  "field.maxTokens": "Max Output Tokens",
  "preset.select": "Select preset",
  "preset.custom": "Custom",
};

const locales: Record<string, Record<string, string>> = { zh, en };

let currentLocale = localStorage.getItem("cplan-locale") || "zh";

export function t(key: string): string {
  return locales[currentLocale]?.[key] ?? locales.zh?.[key] ?? key;
}

export function setLocale(locale: string) {
  currentLocale = locale;
  localStorage.setItem("cplan-locale", locale);
}

export function getLocale(): string {
  return currentLocale;
}
