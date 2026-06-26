# CodexProxy

让 OpenAI Codex 直接调用 Kimi、GLM、DeepSeek 等国产 Coding Plan API 的桌面代理工具。

---

## 背景：Codex 遇到了什么问题？

OpenAI Codex 默认只支持 OpenAI 官方模型。对于国内开发者来说，存在几个现实的痛点：

1. **网络不稳定**：直连 OpenAI API 经常超时或不可用。
2. **成本较高**：OpenAI 官方模型的 token 单价对日常开发并不友好。
3. **国产模型已经很强**：Kimi、GLM、DeepSeek 等提供的 Coding Plan 模型在代码任务上表现优秀，且价格更低、访问更稳定。
4. **配置繁琐**：Codex 使用 OpenAI Responses API，而国产厂商大多提供 Anthropic Messages 格式。即使手动搭建反向代理，也需要维护协议转换、模型映射、API 密钥等配置。

CodexProxy 就是为了解决这些问题而生的：它在本地运行一个轻量级代理，把 Codex 的 Responses 调用翻译成国产厂商能理解的 Anthropic Messages 调用，让你零配置地把 Codex 接到国产 API 上。

---

## 解法：CodexProxy 做了什么？

CodexProxy 在本地 `127.0.0.1:15731` 启动一个 HTTP 代理，并自动完成以下工作：

- **协议翻译**：将 OpenAI Responses API 请求转换为 Anthropic Messages API 请求。
- **模型路由**：根据你选择的供应商，把请求转发到对应的国产 Coding Plan 上游。
- **一键写入 Codex 配置**：自动修改 Codex 的 `config.toml`、`models.json` 和鉴权文件，无需手动编辑。
- **可视化供应商管理**：添加、编辑、删除、排序供应商，一键测试连通性。
- **系统托盘**：最小化到托盘，右键快速切换模型、启停代理。
- **开机自启**：可选随系统启动，保持代理常驻。

---

## 比较优势

| 方案 | 易用性 | 多供应商切换 | 协议转换 | 自动配置 Codex | 系统托盘 |
|------|--------|--------------|----------|----------------|----------|
| 手动改 Codex config + 自建代理 | 低 | 差 | 需自己写 | 手动 | 无 |
| 单一厂商的命令行代理 | 中 | 无 | 部分 | 无 | 无 |
| **CodexProxy** | **高** | **支持** | **内置** | **一键** | **支持** |

相比其他方案，CodexProxy 的优势在于：

- **零命令行**：图形界面完成所有配置，适合不想折腾 config 文件的开发者。
- **多供应商管理**：可以同时配置 Kimi、GLM、DeepSeek 等，随时切换。
- **自动协议适配**：你不需要关心 Responses 和 Anthropic Messages 的差异。
- **热切换模型**：通过托盘或主界面切换模型后，代理自动重启并生效。
- **跨平台**：Windows、macOS、Linux 均有安装包。

---

## 安装

下载最新 [Release](https://github.com/NeoMei/CodexProxy/releases)：

- **Windows**：`CodexProxy_1.0.0_x64.msi`（安装程序会根据系统语言自动选择界面语言）
- **macOS**：`CodexProxy_1.0.0_aarch64.dmg`（Apple Silicon；Intel 设备可尝试通过 Rosetta 运行）
- **Linux**：`CodexProxy_1.0.0_amd64.deb`

> 提示：安装包文件名中的 `1.0.0` 是 Tauri 应用版本号，GitHub Release 版本号可能不同，以 Release 页面为准。

### macOS 额外步骤

由于应用未经过 Apple 公证，首次打开可能会提示「无法打开」。在终端执行：

```bash
xattr -cr /Applications/CodexProxy.app
```

然后重新打开应用。

---

## 使用方法

### 1. 启动 CodexProxy

打开应用后，界面分为左右两部分：

- **左侧**：供应商列表，显示名称、模型、验证状态。
- **右侧**：代理控制、Codex 配置预览、开机自启开关。

### 2. 添加供应商

点击「+ Add Provider」，填写以下信息：

| 供应商 | Upstream URL | Model |
|--------|--------------|-------|
| Kimi | `https://api.kimi.com/coding/v1` | `kimi-for-coding` |
| GLM | `https://open.bigmodel.cn/api/anthropic/v1` | `glm-5.2` |
| DeepSeek | `https://api.deepseek.com/anthropic/v1` | `deepseek-v4-pro` |

> 具体模型名称请以各平台官方文档为准。

### 3. 测试连通性

点击供应商右侧的 🔍 按钮。如果返回 `✓`，说明 API Key 和上游地址正确。

### 4. 启动代理

点击「Start Proxy」。CodexProxy 会在 `127.0.0.1:15731` 启动本地代理。

### 5. 应用到 Codex

选择你想要使用的供应商，点击「Apply to Codex」。CodexProxy 会自动：

- 把代理地址和模型写入 Codex 配置；
- 生成 `models.json` 模型列表；
- 写入鉴权文件。

### 6. 在 Codex Desktop 中使用

打开 Codex Desktop，在模型选择器中切换到你刚刚 Apply 的模型，即可开始使用国产 Coding Plan API。

### 7. 系统托盘快捷操作

关闭主窗口后，CodexProxy 会最小化到系统托盘。右键托盘图标可以：

- 显示/隐藏主界面
- 在「Models」子菜单中快速切换当前模型
- 启动/停止代理
- 退出应用

---

## 原理

```
┌─────────────────┐     Responses API      ┌─────────────────────┐     Anthropic Messages     ┌────────────────────┐
│  Codex Desktop  │  ───────────────────▶  │   CodexProxy        │  ───────────────────────▶  │  Kimi / GLM / ...  │
│                 │  http://127.0.0.1:15731│  (127.0.0.1:15731)  │                            │  Coding Plan API   │
└─────────────────┘                        └─────────────────────┘                            └────────────────────┘
                                                   │
                                                   ▼
                                          自动写入 Codex config.toml
                                          自动写入 models.json
                                          自动写入鉴权文件
```

Codex 以为自己仍在调用 OpenAI Responses API，实际上 CodexProxy 在中间完成了协议转换和模型路由。

---

## 开发

要求：Node 18+、Rust 1.77+

```bash
npm install
npm run tauri dev     # 开发模式
npm run tauri build   # 生产构建
```

---

## License

MIT
