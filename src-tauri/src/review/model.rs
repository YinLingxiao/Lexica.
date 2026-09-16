use serde::Serialize;

use crate::domain::{ExampleId, SenseId, WordId};
use crate::memory::model::{MemoryState, RecallQuality};

/// 题目类型（显式字段，前端不再凭 example_id 推断）。
/// 出题优先级：词典例句挖空 → AI 例句挖空 → 英文释义 → 中文释义兜底。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    /// 例句挖空（词典或 AI 补充）
    Cloze,
    /// 英文释义填词
    Definition,
    /// 中文释义兜底
    DefinitionZh,
}

/// 题目来源：词典原文或 AI 补充（AI 题目不覆盖词典原文）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptSource {
    Dictionary,
    Ai,
}

/// 复习题目条目。**不包含答案**——词形与释义只在提示/判分时由服务端给出。
#[derive(Debug, Clone, Serialize)]
pub struct ReviewItem {
    pub word_id: WordId,
    /// 挖空句或释义文本（按 kind 解释）
    pub prompt: String,
    pub kind: PromptKind,
    pub source: PromptSource,
    pub sense_id: Option<SenseId>,
    /// AI 题目恒为 None——不伪造词典例句 ID
    pub example_id: Option<ExampleId>,
}

/// 逐级提示（spec §12）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Hint {
    /// 1: m_________（首字母 + 定长掩码）
    Mask { text: String },
    /// 2: 英英释义
    English { text: String },
    /// 3: 中文释义（最后一级）
    Chinese { text: String },
}

/// 提交后的反馈：判分 + 答案揭示 + 更新后的记忆状态。
#[derive(Debug, Clone, Serialize)]
pub struct ReviewFeedback {
    pub correct: bool,
    pub quality: RecallQuality,
    /// 正确答案（提交后揭示）
    pub answer: String,
    pub hints_used: u32,
    pub memory: Option<MemoryState>,
}
