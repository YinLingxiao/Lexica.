-- 0005_study_system · 背单词系统与词典 8 大拓展栏数据表

ALTER TABLE words ADD COLUMN tags TEXT NOT NULL DEFAULT '';
ALTER TABLE words ADD COLUMN collins INTEGER;
ALTER TABLE words ADD COLUMN oxford INTEGER NOT NULL DEFAULT 0;

ALTER TABLE word_forms ADD COLUMN kind TEXT NOT NULL DEFAULT 'form';

CREATE TABLE word_enrichments (
  word_id                      INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
  mnemonic                     TEXT,
  roots_json                   TEXT NOT NULL DEFAULT '[]',
  exam_example_text            TEXT,
  exam_example_translation     TEXT,
  standard_example_text        TEXT,
  standard_example_translation TEXT,
  updated_at                   TEXT NOT NULL
);

CREATE TABLE custom_wordbook_words (
  word_id  INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
  added_at TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_words_tags ON words(tags) WHERE tags <> '';
