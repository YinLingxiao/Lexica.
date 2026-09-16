-- AI 例句补充（可选功能）的本地存储。
-- app_settings：非敏感配置（开关/服务地址/模型）。密钥存 Windows 凭据管理器，绝不入库。
-- ai_example_cache：有效例句缓存。键 = 词 + 义项内容 + 服务地址 + 模型 + 提示词版本，
-- 命中可跳过网络请求；词条本身删除时随之清理。

CREATE TABLE app_settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE ai_example_cache (
  cache_key   TEXT PRIMARY KEY,
  word_id     INTEGER NOT NULL REFERENCES words(id) ON DELETE CASCADE,
  sense_id    INTEGER,
  prompt_text TEXT NOT NULL,           -- 已挖空目标词的题目文本
  created_at  TEXT NOT NULL
);
