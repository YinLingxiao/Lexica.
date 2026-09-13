# Lexica

本地优先的英语词典：查词是理解的开始，逐渐不再需要查这个词，才是目的。

Tauri 2 · Rust · SQLite / FTS5 · SvelteKit 5。词库、收藏、笔记和学习记录保存在本机。

## 功能

- 探索词典：前缀建议、词形回退、键盘导航、查词历史、词条深链接。
- 阅读：循序理解 / 完整词条、双语释义、例句高亮、搭配、近反义词跳转、复制词条。
- 我的词汇：收藏、个人笔记、搜索、记忆阶段筛选、分页、暂停 / 恢复提醒。
- 温故知新：最多 12 题的到期复习、三级提示、提交防重、反馈与完成摘要。
- 学习洞察：14 天查词与复习活动、本地日期数据表、记忆阶段分布。
- 偏好设置：浅色 / 深色 / 系统主题、阅读字号、查词模式、ECDICT 导入。
- 发音使用系统已安装的离线英语语音；未安装时显示不可用状态。

## 开发

需要 Node.js、Rust MSVC 工具链、Windows C++ 构建工具与 WebView2。

```bash
npm install
npm run tauri dev
```

单独运行 `npm run dev` 可打开浏览器只读预览。它使用 50 个内置种子词，不能保存学习记录；真实功能请在 Tauri 桌面程序中使用。生产包不包含浏览器预览实现。

## 构建

```bash
npm run build
cd src-tauri
cargo build --release --features custom-protocol --bin lexica
```

Windows 可执行文件：`src-tauri/target/release/lexica.exe`。`custom-protocol` 保证前端资源内嵌，不依赖开发服务器。

需要安装包时运行 `npm run tauri build`。

## 测试

```bash
npm run check
npm run build
npx playwright install chromium
npm run test:e2e
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

界面测试使用只读预览和隔离的 IPC 夹具；Rust 测试验证真实 SQLite 存储和记忆规则。界面截图保存在 `docs/screenshots/`。`npm run format` 统一前端和测试代码格式。

## 数据与架构

- 字典与记忆业务规则在 Rust，前端负责展示、交互状态和外观偏好。
- `domain` → `dictionary` / `memory` / `review`，`commands` 是 IPC 组合层。
- `library` 管理收藏、笔记、个人词汇查询与活动统计。
- `encounters` 和已完成的 `reviews` 为学习事实源，`memory_states` 为可重建投影。
- schema v3 新增 `word_notes`，不删除旧词库或历史记录。
- 默认数据库位于应用数据目录；其中的 `db_dir.txt` 可指向其他数据目录，重启后生效。
- 浏览器外观偏好与桌面应用偏好各自独立；个人词汇数据始终在桌面 SQLite 中。

完整审查、已落实事项、验证边界和后续优先级见 [项目审查报告](docs/PROJECT_REVIEW.md)。
