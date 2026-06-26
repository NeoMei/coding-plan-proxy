# CodexProxy

桌面端管理工具，让 OpenAI Codex 无缝接入 Kimi、GLM、DeepSeek 等国产 Coding Plan API。

## 功能

- **可视化供应商管理** — 添加、编辑、删除 API 供应商，一键测试连通性
- **代理服务** — 内置 Responses → Anthropic Messages 协议翻译，自动启停
- **一键配置 Codex** — 选择模型 → 点 Apply → Codex 立即可用，无需手动编辑 config.toml
- **系统托盘** — 最小化到托盘，右键切换模型、启停代理
- **开机自启** — 设置后随系统启动，Codex 始终可用
- **三平台支持** — Windows、macOS、Linux

## 截图

（启动后效果）
- 左侧：供应商列表，显示名称、模型名、状态，可测试/编辑/删除
- 右侧：代理控制栏（启停）、Codex 配置预览、开机自启开关
- 系统托盘：快速切换模型

## 安装

下载最新 [Release](https://github.com/NeoMei/CodexProxy/releases)：

- **Windows**: `CodexProxy_1.0.0_x64_zh-CN.msi`
- **macOS**: `CodexProxy_1.0.0_aarch64.dmg`（需执行 `xattr -cr /Applications/CodexProxy.app` 解除隔离）
- **Linux**: `CodexProxy_1.0.0_amd64.deb`

## 使用

1. 打开 CodexProxy
2. 点击 "+ Add Provider"，填入供应商信息
   - Kimi: upstream `https://api.kimi.com/coding/v1`，model `kimi-for-coding`
   - GLM: upstream `https://open.bigmodel.cn/api/anthropic/v1`，model `glm-5.2`
   - DeepSeek: upstream `https://api.deepseek.com/anthropic/v1`，model `deepseek-v4-pro`
3. 点击🔍测试连通性
4. 点击 "Start Proxy" 启动代理
5. 点击 "Apply to Codex" 将模型写入 Codex 配置
6. 打开 Codex Desktop → 模型选择器中直接切换

## 原理

```
Codex (Responses API)
  → Coding Plan Proxy (127.0.0.1:15731)
    → 协议翻译：Responses → Anthropic Messages
    → Kimi / GLM / DeepSeek (Anthropic API)
```

## 开发

```bash
# 要求：Node 18+, Rust 1.77+, pnpm/npm
npm install
npm run tauri dev     # 开发模式
npm run tauri build   # 生产构建
```

## License

MIT
