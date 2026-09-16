# Lexica

> 本地优先的英语词典：查词是理解的开始，逐渐不再需要查这个词，才是目的。

[![release](https://img.shields.io/github/v/release/YinLingxiao/Lexica.)](https://github.com/YinLingxiao/Lexica./releases/latest)
[![downloads](https://img.shields.io/github/downloads/YinLingxiao/Lexica./total)](https://github.com/YinLingxiao/Lexica./releases)
![license](https://img.shields.io/github/license/YinLingxiao/Lexica.)
![platform](https://img.shields.io/badge/platform-Windows%20x64-8a5a2b)
![stack](https://img.shields.io/badge/Tauri%202-Rust%20%C2%B7%20SvelteKit%205-2f6f4e)

**Tauri 2 · Rust · SQLite / FTS5 · SvelteKit 5**

词库、收藏、笔记和学习记录全部保存在本机。除可选的 AI 例句补充外，应用不发起任何网络请求。

![Lexica 首页](docs/screenshots/home-light.png)

---

## 下载

Windows 10 / 11（x64），需要 WebView2 运行时（系统通常已预装）。

| 文件 | 大小 | 说明 |
| --- | --- | --- |
| **[Lexica-setup.exe](https://github.com/YinLingxiao/Lexica./releases/latest/download/Lexica-setup.exe)** | ~3.5 MB | **推荐**。双击安装，自动创建开始菜单与桌面快捷方式 |
| [Lexica.msi](https://github.com/YinLingxiao/Lexica./releases/latest/download/Lexica.msi) | ~5.3 MB | 适合企业批量部署 / 静默安装 |

上面两个是**永久链接**，始终指向最新版本。也可以在 [Releases](https://github.com/YinLingxiao/Lexica./releases) 页面挑选具体版本。

> ⚠️ 安装包目前**没有代码签名**，Windows 可能提示"未知发布者"。选择「更多信息 → 仍要运行」即可。正式分发建议配置代码签名证书。

---

## 功能

### 探索词典

前缀建议、词形回退（`running` → `run`）、键盘上下选择、查词历史、词条深链接（`/?word=meticulous`）。

### 阅读

「循序理解」与「完整词条」两种密度，双语释义、例句目标词高亮、常见搭配、近反义词一键跳转、整条词条复制。

### 我的词汇

收藏、个人笔记、搜索、按记忆阶段筛选、分页，以及暂停 / 恢复提醒。

### 温故知新

最多 12 题的到期复习，三级递进提示（首字母 → 英文释义 → 中文释义）、提交防重、即时反馈与完成摘要。答错的词会重新排入本轮，重试不会重复计入正式记录。

### AI 例句补充（可选，默认关闭）

- 查词时若词典本身没有例句，补一条 AI 生成的例句；
- 复习的填空句统一由 AI 生成，**判分永远在本地**完成。

默认对接 DeepSeek（`https://api.deepseek.com/v1` + `deepseek-chat`），也可换成任何 OpenAI 兼容服务。

安全与隐私设计：

- API Key 存在 **Windows 凭据管理器**，不写入数据库、不写 localStorage、不进日志；
- 发到前端的配置视图只报告"是否已配置密钥"，**绝不回传密钥本身**；
- 生成结果在 Rust 侧校验（义项 ID 命中、句子非空、目标词完整词边界出现）并挖空后才交给前端，不合规条目直接丢弃并回退本地题目；
- 有效例句按「词 + 义项 + 服务地址 + 模型 + 提示词版本」缓存，命中即跳过网络请求；
- 单次请求 15 秒超时，**不自动重试**——失败即回退本地题目，不阻塞复习；
- 等待网络期间不持有数据库锁。

### 学习洞察

14 天查词与复习活动、本地日期数据表、记忆阶段分布。

### 偏好设置

浅色 / 深色 / 系统主题（带圆形扩散过渡动画）、阅读字号、查词模式、ECDICT 导入、AI 例句配置。

### 发音

使用系统已安装的离线英语语音；未安装时明确显示为不可用，而不是静默失败。

---

## 截图

| | |
| --- | --- |
| ![首页](docs/screenshots/home-light.png) | ![词条](docs/screenshots/word-light.png) |
| 首页：搜索优先的布局 | 词条：完整词条视图 |
| ![深色](docs/screenshots/word-dark.png) | ![复习](docs/screenshots/review-fixture.png) |
| 深色主题 + 大字号 | 复习：三级提示全部展开 |
| ![AI 设置](docs/screenshots/ai-settings.png) | ![移动端](docs/screenshots/settings-mobile.png) |
| 设置：AI 例句补充面板 | 设置：480px 窄窗口适配 |

> 截图取自**浏览器只读预览**（内置 50 个种子词，显示为 `50 entries · offline`）。桌面程序使用真实 ECDICT 词库，视觉与交互一致。
> 截图由 e2e 测试自动生成到 `docs/screenshots/`，不是手工维护的。

---

## 首次使用

1. **词库**：首次启动会引导下载 ECDICT 词库（约 200 MB，SQLite 版）。也可以跳过，在设置里手动导入自己的 `stardict.db`。
2. **发音**：依赖系统离线英语语音。Windows 可在「设置 → 时间和语言 → 语音」里安装英文语音包。
3. **AI 例句**（可选）：在设置里打开开关，填服务地址、模型和 API Key。

---

## 技术栈与架构

业务规则全在 Rust，前端只负责展示、交互状态和外观偏好。

```
src-tauri/src/
├── domain/       纯领域模型与规则（无 IO）
├── dictionary/   词库引擎：FTS5 检索、前缀建议、词形回退、ECDICT provider
├── memory/       记忆阶段投影与推进规则
├── review/       出题、判分、记录（grader / model / service）
├── library.rs    收藏、笔记、个人词汇查询、活动统计
├── ai.rs         AI 例句补充（可选，默认关闭）
├── database/     连接管理、版本化迁移、时间格式
├── commands/     IPC 组合层（Tauri command）
├── app.rs        应用状态与路径解析
└── bin/          无头工具：import-ecdict
```

数据模型要点：

- `encounters` 与已完成的 `reviews` 是**学习事实源**；`memory_states` 是**可重建投影**，随时能由事实源重算。
- 迁移按序号顺序应用，`PRAGMA user_version` 记录版本。当前 schema **v4**：
  `0001_init` → `0002_freq_rank_index` → `0003_word_notes` → `0004_ai_examples`。
- 迁移只增不删，不会破坏旧词库或历史记录。
- 默认数据库位于应用数据目录；其中的 `db_dir.txt` 可指向其他数据目录，**重启后生效**。

前端：

```
src/
├── routes/       SvelteKit 路由：/ · /library · /review · /stats · /settings
├── lib/api.ts    Tauri IPC 封装（浏览器预览时走只读夹具）
├── lib/preview.ts 浏览器只读预览实现（生产包不含）
└── lib/components/
```

浏览器外观偏好与桌面应用偏好各自独立；个人词汇数据始终只存在桌面 SQLite 中。

---

## 开发

需要 **Node.js**、**Rust MSVC 工具链**、**Windows C++ 构建工具**与 **WebView2**。

```bash
npm install
npm run tauri dev
```

只跑前端：

```bash
npm run dev
```

`npm run dev` 打开的是**浏览器只读预览**，使用 50 个内置种子词，不能保存学习记录；真实功能请在 Tauri 桌面程序中使用。生产包不包含浏览器预览实现。

---

## 构建

### 独立可执行文件

```bash
npm run build
cd src-tauri
cargo build --release --features custom-protocol --bin lexica
```

产物：`src-tauri/target/release/lexica.exe`。

`custom-protocol` 特性在 release 构建中**必须启用**——`generate_context!` 靠它区分 dev（从 devUrl 热加载）与 release（把 `build/` 前端资产内嵌进二进制），否则产物不依赖开发服务器就无法运行。

### Windows 安装包

```bash
npm install
npm run tauri build
```

产物：

- `src-tauri/target/release/bundle/msi/Lexica_0.1.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/Lexica_0.1.0_x64-setup.exe`

版本号和产品名来自 `src-tauri/tauri.conf.json`，发布新版本前先改其中的 `version`。

### 无头词库导入

不启动 GUI，直接把 ECDICT 导入目标库（与应用内导入走同一条 `EcdictProvider` 代码路径）：

```bash
cargo run --release --bin import-ecdict -- <stardict.db> <lexica.db>
```

目标库不存在时自动创建并迁移；已导入过则幂等跳过（0 新增）。

---

## 发布

推一个 `v*` 标签即可，GitHub Actions 会自动在 Windows 上构建、跑检查、创建 Release 并上传安装包：

```bash
git tag v0.2.0
git push origin v0.2.0
```

工作流见 [`.github/workflows/release.yml`](.github/workflows/release.yml)。除了带版本号的产物，它还会额外上传一份固定文件名的副本（`Lexica-setup.exe` / `Lexica.msi`），这样 `releases/latest/download/<固定名>` 永远指向最新版本，下载页不需要随版本改链接。

---

## 测试

```bash
npm run check                                  # svelte-check：0 错误 0 警告
npm run build
npx playwright install chromium
npm run test:e2e                               # 28 个界面测试
cd src-tauri
cargo test                                     # 77 个 Rust 测试
cargo clippy --all-targets -- -D warnings
```

- **界面测试**（Playwright）覆盖只读预览与隔离的 IPC 夹具：键盘选词、三级提示顺序、防重复提交、错词重排、主题过渡动画、AI 回退与缓存、窄窗口适配等。
- **Rust 测试**验证真实 SQLite 存储与记忆规则。
- 界面截图保存在 `docs/screenshots/`，由测试自动生成。
- `npm run format` 统一前端和测试代码格式。

---

## 数据与隐私

- 词库、收藏、笔记、复习记录全部保存在**本机** SQLite。
- 除可选的 AI 例句补充外，**不发起任何网络请求**。
- 开启 AI 例句后，只会把当前单词与释义发送到**你自己配置**的服务地址；判分始终在本地。
- API Key 存 Windows 凭据管理器，应用绝不记录或回传密钥。

---

## 已知限制

- 仅提供 **Windows x64** 版本（MSI / NSIS），暂无 macOS 与 Linux 构建。
- 安装包**未做代码签名**，SmartScreen 会提示"未知发布者"。
- 发音依赖系统已安装的离线英语语音。
- 词库首次导入需要约 200 MB 下载，或自备 `stardict.db`。
- AI 例句补充需要自备第三方 API Key，会产生相应费用。

---

## 许可证

[MIT](LICENSE) © YinLingxiao

你可以自由使用、修改、分发本项目，包括商业用途，只需保留版权声明与许可证副本。

---

## 致谢

完整审查、已落实事项、验证边界和后续优先级见 [项目审查报告](docs/PROJECT_REVIEW.md)。

设置页署名图标点击后跳转至 [github.com/YinLingxiao](https://github.com/YinLingxiao)。
