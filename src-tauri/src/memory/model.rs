use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::domain::{ComprehensionLevel, EncounterId, WordId};

/// 接触来源。v1 只有查词；review 预留（复习本身由 reviews 表记录）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterSource {
    Lookup,
    #[allow(dead_code)]
    Review,
}

impl EncounterSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lookup => "lookup",
            Self::Review => "review",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "lookup" => Self::Lookup,
            "review" => Self::Review,
            _ => return None,
        })
    }
}

/// 一次查词访问。查词本身就是 Encounter——用户不需要维护生词本。
#[derive(Debug, Clone, Serialize)]
pub struct Encounter {
    pub id: EncounterId,
    pub word_id: WordId,
    pub timestamp: DateTime<Utc>,
    pub source: EncounterSource,
    pub comprehension_level: ComprehensionLevel,
}

/// Recent 列表条目（不含词文本——词文本由 commands 从 dictionary 补齐，保持解耦）。
#[derive(Debug, Clone, Serialize)]
pub struct RecentVisit {
    pub word_id: WordId,
    pub visited_at: DateTime<Utc>,
    pub comprehension_level: ComprehensionLevel,
}

/// 复习质量（spec §12 的提示→质量映射）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallQuality {
    Excellent,
    Good,
    Hard,
    Poor,
    Forgotten,
}

impl RecallQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Excellent => "excellent",
            Self::Good => "good",
            Self::Hard => "hard",
            Self::Poor => "poor",
            Self::Forgotten => "forgotten",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "excellent" => Self::Excellent,
            "good" => Self::Good,
            "hard" => Self::Hard,
            "poor" => Self::Poor,
            "forgotten" => Self::Forgotten,
            _ => return None,
        })
    }

    /// 序号：Poor=1 … Excellent=4；Forgotten=0（单独分支处理）。
    pub fn index(self) -> usize {
        match self {
            Self::Poor => 1,
            Self::Hard => 2,
            Self::Good => 3,
            Self::Excellent => 4,
            Self::Forgotten => 0,
        }
    }
}

/// 折算用事件（encounters + 已完成 reviews 的统一只读视图）。
/// 每次 visit 的级别取**当前值**：visit 内级别会随披露动作升级，
/// 每次变更都会触发该词重折，折算天然自洽（无需 pending 概念）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryEvent {
    Visit {
        at: DateTime<Utc>,
        level: ComprehensionLevel,
    },
    ReviewDone {
        at: DateTime<Utc>,
        quality: RecallQuality,
    },
}

/// 词汇掌握桶（统计展示用）。按 strength_days 现算，不落库。
pub const FAMILIAR_THRESHOLD: f64 = 4.0;
pub const STABLE_THRESHOLD: f64 = 21.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Learning,
    Familiar,
    Stable,
}

impl MemoryStatus {
    pub fn from_strength(strength_days: f64) -> Self {
        if strength_days < FAMILIAR_THRESHOLD {
            Self::Learning
        } else if strength_days < STABLE_THRESHOLD {
            Self::Familiar
        } else {
            Self::Stable
        }
    }
}

/// 记忆状态投影。retrievability 不落库，读时由 scheduler 现算。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryState {
    pub word_id: WordId,
    pub strength_days: f64,
    pub difficulty: f64,
    pub review_count: u32,
    pub lapse_count: u32,
    pub visit_count: u32,
    pub weak_streak: u32,
    pub high_priority: bool,
    pub first_visit_at: DateTime<Utc>,
    pub last_visit_at: Option<DateTime<Utc>>,
    pub last_review_at: Option<DateTime<Utc>>,
    pub last_reinforcement_at: Option<DateTime<Utc>>,
    pub next_review_at: Option<DateTime<Utc>>,
}

impl MemoryState {
    pub fn status(&self) -> MemoryStatus {
        MemoryStatus::from_strength(self.strength_days)
    }
}
