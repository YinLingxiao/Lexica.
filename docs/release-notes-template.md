<!-- Release 说明模板。发布脚本会把 {{VERSION}} 与 {{TAG}} 替换成实际值。 -->

Lexica {{TAG}}：本地优先的英语词典 + 间隔复习。

词库、收藏、笔记和学习记录全部保存在本机。

![Lexica 首页](https://raw.githubusercontent.com/YinLingxiao/Lexica./main/docs/screenshots/home-light.png)

## 下载

| 文件 | 说明 |
| --- | --- |
| **[Lexica-setup.exe](https://github.com/YinLingxiao/Lexica./releases/latest/download/Lexica-setup.exe)** | **推荐**。双击安装，自动创建开始菜单与桌面快捷方式 |
| [Lexica.msi](https://github.com/YinLingxiao/Lexica./releases/latest/download/Lexica.msi) | 适合企业批量部署 / 静默安装 |

带版本号的同名文件（`Lexica_{{VERSION}}_x64-*`）内容完全一致，只是方便确认版本。
上面两个不带版本号的是**永久链接**，始终指向最新版本。

## 这个版本包含

- **探索词典**：前缀建议、词形回退、键盘导航、查词历史、词条深链接
- **阅读**：循序理解 / 完整词条、双语释义、例句高亮、搭配、近反义词跳转
- **我的词汇**：收藏、个人笔记、搜索、记忆阶段筛选、分页、暂停 / 恢复提醒
- **温故知新**：最多 12 题的到期复习、三级提示、提交防重、反馈与完成摘要
- **AI 例句补充**（可选，默认关闭）：查词缺例句时补 AI 例句，复习填空句由 AI 生成
- **学习洞察**：14 天查词与复习活动、记忆阶段分布
- **偏好设置**：浅色 / 深色 / 系统主题、阅读字号、查词模式、ECDICT 导入

## 界面

| | |
| --- | --- |
| ![词条](https://raw.githubusercontent.com/YinLingxiao/Lexica./main/docs/screenshots/word-light.png) | ![深色](https://raw.githubusercontent.com/YinLingxiao/Lexica./main/docs/screenshots/word-dark.png) |
| 完整词条视图 | 深色主题 + 大字号 |
| ![复习](https://raw.githubusercontent.com/YinLingxiao/Lexica./main/docs/screenshots/review-fixture.png) | ![AI 设置](https://raw.githubusercontent.com/YinLingxiao/Lexica./main/docs/screenshots/ai-settings.png) |
| 复习：三级提示全部展开 | 设置：AI 例句补充面板 |

> 截图取自浏览器只读预览（内置 50 个种子词）。桌面程序使用真实 ECDICT 词库，视觉与交互一致。

## 首次使用

1. 首次启动会引导下载 ECDICT 词库（约 200 MB）；也可手动导入自己的 `stardict.db`。
2. 发音使用系统已安装的离线英语语音；未安装时显示为不可用状态。
3. AI 例句补充默认关闭。需要时在设置里填服务地址、模型和 API Key。
   **API Key 存在 Windows 凭据管理器**，不写入数据库、不写日志。

## 隐私

除开启 AI 例句补充外，应用不发送任何网络请求。开启后，仅把当前单词与释义发送到你自己配置的服务地址；判分始终在本地完成。

## 已知限制

- 安装包**未做代码签名**，Windows 可能提示"未知发布者"。选择「更多信息 → 仍要运行」即可。
- 仅提供 **Windows x64** 版本。
- 需要 WebView2 运行时（Windows 10/11 通常已预装）。
