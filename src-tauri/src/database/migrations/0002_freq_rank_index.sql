-- 340 万词条规模下，词头前缀建议不再走 FTS（每义项一行 + 全量排序，
-- 单字母前缀 600ms+），改走 words 表：本索引支撑"按词频取常用词"，
-- 字母序补齐则复用已有的 idx_words_word 范围扫描。
CREATE INDEX idx_words_freq_rank ON words(frequency_rank) WHERE frequency_rank IS NOT NULL;
