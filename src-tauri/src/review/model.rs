use serde::Serialize;

use crate::domain::{ExampleId, SenseId, WordId};
use crate::memory::model::{MemoryState, RecallQuality};

/// 复习队列条目。**不包含答案**——词形与释义只在提示/判分时由服务端给出。
#[derive(Debug, Clone, Serialize)]
pub struct ReviewItem {
    pub word_id: WordId,
    /// cloze 挖空句；无可用例句时降级为英英释义
    pub prompt: String,
    pub sense_id: Option<SenseId>,
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
