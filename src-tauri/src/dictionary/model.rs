use serde::{Deserialize, Serialize};

use crate::domain::{ExampleId, SenseId, WordId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WordFormEntry {
    pub label: String, // 如 "Plural", "Past", "Past participle", "Present participle", "3rd person singular", "Comparative", "Superlative"
    pub form: String,  // 如 "surgeries"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelatedWordItem {
    pub word: String,          // 如 "microsurgery" 或 "sugary"
    pub pos: Option<String>,   // 如 Some("n.".into())
    pub gloss: String,         // 中文简明释义，如 "显微外科"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RootPart {
    pub part: String,          // 如 "surg" 或 "-ory"
    pub meaning: String,       // 如 "上升，升起" 或 "构成名词，表示‘...的地方’或‘与...相关的事物’"
}

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
    pub tags: Vec<String>,
    pub collins: Option<u8>,
    pub oxford: bool,
    pub mnemonic: Option<String>,
    pub roots: Vec<RootPart>,
    pub derivatives: Vec<RelatedWordItem>,
    pub synonyms_detailed: Vec<RelatedWordItem>,
    pub antonyms_detailed: Vec<RelatedWordItem>,
    pub lookalikes: Vec<RelatedWordItem>,
    pub forms: Vec<WordFormEntry>,
    pub exam_example: Option<Example>,
}

impl Default for WordEntry {
    fn default() -> Self {
        Self {
            id: WordId(0),
            word: String::new(),
            display: String::new(),
            phonetic: None,
            senses: Vec::new(),
            collocations: Vec::new(),
            synonyms: Vec::new(),
            antonyms: Vec::new(),
            word_family: Vec::new(),
            frequency_rank: None,
            tags: Vec::new(),
            collins: None,
            oxford: false,
            mnemonic: None,
            roots: Vec::new(),
            derivatives: Vec::new(),
            synonyms_detailed: Vec::new(),
            antonyms_detailed: Vec::new(),
            lookalikes: Vec::new(),
            forms: Vec::new(),
            exam_example: None,
        }
    }
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
#[allow(clippy::large_enum_variant)]
pub enum LookupOutcome {
    Hit { entry: WordEntry },
    Miss { suggestions: Vec<Suggestion> },
}
