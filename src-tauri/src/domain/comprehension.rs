use serde::{Deserialize, Serialize};

/// 一次接触（visit）中，用户理解词义所需的帮助级别。
/// 这是记忆系统最重要的输入信号。
///
/// 偏序（按帮助量）：`Context < EnglishDefinition < ChineseDefinition`。
/// `Unknown` 是 visit 的初值（尚未披露）；参与记忆折算时视同 Chinese（仍不懂）。
///
/// 产品原则：Chinese is the fallback, not the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComprehensionLevel {
    /// 仅凭例句即理解。
    #[serde(rename = "context")]
    Context,
    /// 需要英英释义。
    #[serde(rename = "english")]
    EnglishDefinition,
    /// 需要中文释义（最后一级提示）。
    #[serde(rename = "chinese")]
    ChineseDefinition,
    /// 尚未披露 / 仍不理解。
    #[serde(rename = "unknown")]
    Unknown,
}

impl ComprehensionLevel {
    /// 帮助量序：值越大 = 需要的帮助越多。
    /// Unknown 与 Chinese 同级（折算语义），但 escalate 判定单独处理。
    pub fn help_rank(self) -> u8 {
        match self {
            Self::Context => 0,
            Self::EnglishDefinition => 1,
            Self::ChineseDefinition | Self::Unknown => 2,
        }
    }

    /// 数据库存储字符串，与 encounters.comprehension_level 的 CHECK 约束一致。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::EnglishDefinition => "english",
            Self::ChineseDefinition => "chinese",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "context" => Self::Context,
            "english" => Self::EnglishDefinition,
            "chinese" => Self::ChineseDefinition,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_rank_orders_by_assistance_needed() {
        assert!(
            ComprehensionLevel::Context.help_rank()
                < ComprehensionLevel::EnglishDefinition.help_rank()
        );
        assert!(
            ComprehensionLevel::EnglishDefinition.help_rank()
                < ComprehensionLevel::ChineseDefinition.help_rank()
        );
    }

    #[test]
    fn serde_uses_short_ipc_names() {
        assert_eq!(
            serde_json::to_string(&ComprehensionLevel::Context).unwrap(),
            r#""context""#
        );
        assert_eq!(
            serde_json::to_string(&ComprehensionLevel::EnglishDefinition).unwrap(),
            r#""english""#
        );
        assert_eq!(
            serde_json::to_string(&ComprehensionLevel::ChineseDefinition).unwrap(),
            r#""chinese""#
        );
        assert_eq!(
            serde_json::to_string(&ComprehensionLevel::Unknown).unwrap(),
            r#""unknown""#
        );
    }

    #[test]
    fn db_roundtrip() {
        for level in [
            ComprehensionLevel::Context,
            ComprehensionLevel::EnglishDefinition,
            ComprehensionLevel::ChineseDefinition,
            ComprehensionLevel::Unknown,
        ] {
            assert_eq!(ComprehensionLevel::from_db(level.as_str()), Some(level));
        }
        assert_eq!(ComprehensionLevel::from_db("nonsense"), None);
    }
}
