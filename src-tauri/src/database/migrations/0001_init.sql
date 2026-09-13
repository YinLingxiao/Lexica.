-- ══════════════════════════════════════════════════════════════════
-- 0001_init · Lexica 初始 schema
--
-- 事实源 = encounters + reviews + review_attempts（事件流）
-- memory_states 是纯投影，可随时从事件流全量重建（见 memory::scheduler）。
-- 时间戳一律 UTC、RFC3339 定宽毫秒 TEXT（字典序可排序）。
-- ══════════════════════════════════════════════════════════════════

-- ── 词库 ──────────────────────────────────────────────────────────

CREATE TABLE words (
  id             INTEGER PRIMARY KEY,
  word           TEXT NOT NULL,                -- 规范化（小写、NFC、直引号）
  display        TEXT NOT NULL,
  phonetic       TEXT,
  frequency_rank INTEGER,
  synonyms       TEXT NOT NULL DEFAULT '[]',   -- JSON 数组，仅展示
  antonyms       TEXT NOT NULL DEFAULT '[]',
  word_family    TEXT NOT NULL DEFAULT '[]',
  source         TEXT NOT NULL DEFAULT 'seed', -- seed|ecdict|user|...
  created_at     TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_words_word ON words(word);

CREATE TABLE senses (
  id                 INTEGER PRIMARY KEY,
  word_id            INTEGER NOT NULL REFERENCES words(id) ON DELETE CASCADE,
  pos                TEXT,
  english_definition TEXT NOT NULL,
  chinese_definition TEXT,
  level              TEXT,                      -- CEFR 标签 A1..C2（可空）
  order_idx          INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_senses_word ON senses(word_id);

CREATE TABLE examples (
  id          INTEGER PRIMARY KEY,
  sense_id    INTEGER NOT NULL REFERENCES senses(id) ON DELETE CASCADE,
  text        TEXT NOT NULL,
  translation TEXT,
  order_idx   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_examples_sense ON examples(sense_id);

CREATE TABLE collocations (
  id        INTEGER PRIMARY KEY,
  word_id   INTEGER NOT NULL REFERENCES words(id) ON DELETE CASCADE,
  text      TEXT NOT NULL,
  gloss     TEXT,
  order_idx INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_collocations_word ON collocations(word_id);

-- 屈折映射：went→go、better→good（查找回退路径）
CREATE TABLE word_forms (
  form    TEXT NOT NULL,
  word_id INTEGER NOT NULL REFERENCES words(id) ON DELETE CASCADE,
  PRIMARY KEY (form, word_id)
);
CREATE INDEX idx_word_forms_word ON word_forms(word_id);

-- FTS5：每行对应一个 sense（rowid = senses.id），触发器维护同步。
-- 词头前缀搜索用列过滤 `word:quer*`；英文释义全文 `english:...`。
-- 已知边界：unicode61 不切分 CJK，v1 不做中文定义全文搜索。
CREATE VIRTUAL TABLE words_fts USING fts5(
  word, english, chinese,
  tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER senses_ai AFTER INSERT ON senses BEGIN
  INSERT INTO words_fts(rowid, word, english, chinese)
  SELECT new.id, w.word, new.english_definition, new.chinese_definition
  FROM words w WHERE w.id = new.word_id;
END;
CREATE TRIGGER senses_ad AFTER DELETE ON senses BEGIN
  DELETE FROM words_fts WHERE rowid = old.id;
END;
CREATE TRIGGER senses_au AFTER UPDATE ON senses BEGIN
  UPDATE words_fts SET english = new.english_definition, chinese = new.chinese_definition
  WHERE rowid = new.id;
END;
CREATE TRIGGER words_au AFTER UPDATE OF word ON words BEGIN
  UPDATE words_fts SET word = new.word
  WHERE rowid IN (SELECT id FROM senses WHERE word_id = new.id);
END;

-- ── 事件流（记忆的事实源）────────────────────────────────────────

CREATE TABLE encounters (
  id                  INTEGER PRIMARY KEY,
  word_id             INTEGER NOT NULL REFERENCES words(id) ON DELETE CASCADE,
  visited_at          TEXT NOT NULL,
  source              TEXT NOT NULL DEFAULT 'lookup' CHECK (source IN ('lookup','review')),
  comprehension_level TEXT NOT NULL DEFAULT 'unknown'
                      CHECK (comprehension_level IN ('context','english','chinese','unknown'))
);
CREATE INDEX idx_encounters_word_time ON encounters(word_id, visited_at);
CREATE INDEX idx_encounters_time ON encounters(visited_at);

-- ── 记忆投影（可从事件流全量重建）────────────────────────────────

CREATE TABLE memory_states (
  word_id               INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
  strength_days         REAL NOT NULL,
  difficulty            REAL NOT NULL,
  review_count          INTEGER NOT NULL DEFAULT 0,
  lapse_count           INTEGER NOT NULL DEFAULT 0,
  visit_count           INTEGER NOT NULL DEFAULT 0,
  weak_streak           INTEGER NOT NULL DEFAULT 0,
  high_priority         INTEGER NOT NULL DEFAULT 0,
  first_visit_at        TEXT NOT NULL,
  last_visit_at         TEXT,
  last_review_at        TEXT,
  last_reinforcement_at TEXT,
  next_review_at        TEXT,
  updated_at            TEXT NOT NULL
);
CREATE INDEX idx_memory_due ON memory_states(next_review_at);

-- 用户偏好（不是记忆事件，不参与折算）
CREATE TABLE ignored_words (
  word_id INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE
);

-- ── 复习 ─────────────────────────────────────────────────────────

CREATE TABLE reviews (
  id           INTEGER PRIMARY KEY,
  word_id      INTEGER NOT NULL REFERENCES words(id) ON DELETE CASCADE,
  started_at   TEXT NOT NULL,
  completed_at TEXT,
  quality      TEXT CHECK (quality IN ('excellent','good','hard','poor','forgotten')),
  hints_used   INTEGER NOT NULL DEFAULT 0,
  sense_id     INTEGER REFERENCES senses(id) ON DELETE SET NULL,
  example_id   INTEGER REFERENCES examples(id) ON DELETE SET NULL
);
CREATE INDEX idx_reviews_word_time ON reviews(word_id, started_at);

-- 原始行为日志：每次提示请求 / 提交
CREATE TABLE review_attempts (
  id        INTEGER PRIMARY KEY,
  review_id INTEGER NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
  at        TEXT NOT NULL,
  action    TEXT NOT NULL CHECK (action IN ('hint1','hint2','hint3','answer','reveal')),
  correct   INTEGER                -- answer/reveal 时 0/1；hint 为 NULL
);
CREATE INDEX idx_attempts_review ON review_attempts(review_id);
