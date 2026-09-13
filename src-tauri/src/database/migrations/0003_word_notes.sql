CREATE TABLE word_notes (
  word_id INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
  bookmarked INTEGER NOT NULL DEFAULT 0 CHECK (bookmarked IN (0, 1)),
  note TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_word_notes_bookmarked ON word_notes(bookmarked, updated_at);
CREATE INDEX idx_reviews_completed ON reviews(completed_at) WHERE completed_at IS NOT NULL;
