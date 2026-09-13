use serde::Serialize;

use crate::domain::{ExampleId, SenseId, WordId};

/// 完整词条（查词命中时返回前端）。
#[derive(Debug, Clone, Serialize)]
pub struct WordEntry {
    pub id: WordId,
    pub word: String,
    pub display: String,
    pub phonetic: Option<String>,
    pub senses: Vec<Sense>,
    pub collocations: Vec<Collocation>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub word_family: Vec<String>,
    pub frequency_rank: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sense {
    pub id: SenseId,
    pub pos: Option<String>,
    pub english_definition: String,
    pub chinese_definition: Option<String>,
    /// CEFR 标签 A1..C2（可空）
    pub level: Option<String>,
    pub examples: Vec<Example>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Example {
    pub id: ExampleId,
    pub text: String,
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Collocation {
    pub text: String,
    pub gloss: Option<String>,
}

/// 搜索建议（输入过程中的下拉条目）。
#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub word: String,
    pub display: String,
    pub phonetic: Option<String>,
    pub frequency_rank: Option<u32>,
}

/// commands 层词显示信息（如 Recent 列表补齐词头）。
#[derive(Debug, Clone, Serialize)]
pub struct WordBrief {
    pub word: String,
    pub display: String,
    pub phonetic: Option<String>,
}

/// 查词结果。Miss 时附带前缀建议，前端展示 "did you mean..."。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LookupOutcome {
    Hit { entry: WordEntry },
    Miss { suggestions: Vec<Suggestion> },
}
