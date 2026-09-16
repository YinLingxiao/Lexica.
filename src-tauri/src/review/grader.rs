//! 判分与出题——纯函数，全部可单测。

use crate::dictionary::engine::normalize_word;
use crate::dictionary::model::WordEntry;
use crate::domain::{ExampleId, SenseId};
use crate::memory::model::RecallQuality;

use super::model::PromptKind;

/// 出题产物：题目文本 + 类型 + 出处（sense/example 供提示与留档）。
pub struct Prompt {
    pub kind: PromptKind,
    pub text: String,
    pub sense_id: Option<SenseId>,
    pub example_id: Option<ExampleId>,
}

/// 出题优先级：词典例句挖空 → 英文释义填词 → 中文释义兜底。
/// （AI 例句由 ai 模块验证后经 blank_word 挖空，优先级在应用层拼装。）
pub fn build_prompt(entry: &WordEntry) -> Prompt {
    for sense in &entry.senses {
        for ex in &sense.examples {
            if let Some(blanked) = blank_word(&ex.text, &entry.word) {
                return Prompt {
                    kind: PromptKind::Cloze,
                    text: blanked,
                    sense_id: Some(sense.id),
                    example_id: Some(ex.id),
                };
            }
        }
    }
    if let Some(s) = entry
        .senses
        .iter()
        .find(|s| !s.english_definition.trim().is_empty())
    {
        let definition =
            blank_word(&s.english_definition, &entry.word).unwrap_or_else(|| s.english_definition.clone());
        return Prompt {
            kind: PromptKind::Definition,
            text: definition,
            sense_id: Some(s.id),
            example_id: None,
        };
    }
    build_definition_prompt(entry)
}

/// 复习时的本地兜底：只以释义出题，避免把词典例句当作填空题。
pub fn build_definition_prompt(entry: &WordEntry) -> Prompt {
    if let Some(s) = entry.senses.iter().find(|s| {
        s.chinese_definition
            .as_deref()
            .is_some_and(|d| !d.trim().is_empty())
    }) {
        let zh = s.chinese_definition.clone().unwrap_or_default();
        let definition = blank_word(&zh, &entry.word).unwrap_or(zh);
        return Prompt {
            kind: PromptKind::DefinitionZh,
            text: definition,
            sense_id: Some(s.id),
            example_id: None,
        };
    }
    if let Some(s) = entry
        .senses
        .iter()
        .find(|s| !s.english_definition.trim().is_empty())
    {
        let definition =
            blank_word(&s.english_definition, &entry.word).unwrap_or_else(|| s.english_definition.clone());
        return Prompt {
            kind: PromptKind::Definition,
            text: definition,
            sense_id: Some(s.id),
            example_id: None,
        };
    }
    Prompt {
        kind: PromptKind::Definition,
        text: String::new(),
        sense_id: None,
        example_id: None,
    }
}

/// 在原句中把目标词替换为 "______"（大小写不敏感；不改变句中其余原文）。
/// 找不到 → None。
pub fn blank_word(text: &str, word: &str) -> Option<String> {
    let word_chars: Vec<char> = word.to_lowercase().chars().collect();
    if word_chars.is_empty() {
        return None;
    }
    let text_chars: Vec<char> = text.chars().collect();
    if text_chars.len() < word_chars.len() {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut start = 0;
    let mut found = false;
    while start < text_chars.len() {
        let end = start + word_chars.len();
        if end > text_chars.len() {
            out.extend(text_chars[start..].iter());
            break;
        }
        let starts_at_boundary = start == 0 || !text_chars[start - 1].is_alphanumeric();
        let ends_at_boundary = end == text_chars.len() || !text_chars[end].is_alphanumeric();
        if !starts_at_boundary || !ends_at_boundary {
            out.push(text_chars[start]);
            start += 1;
            continue;
        }
        let matched = (0..word_chars.len()).all(|j| {
            text_chars[start + j]
                .to_lowercase()
                .eq(word_chars[j].to_lowercase())
        });
        if matched {
            out.push_str("______");
            found = true;
            start = end;
        } else {
            out.push(text_chars[start]);
            start += 1;
        }
    }
    found.then_some(out)
}

/// 首字母掩码提示：meticulous → m_________
pub fn mask_hint(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push(chars[0]);
    for _ in 1..chars.len() {
        out.push('_');
    }
    out
}

/// 答案判定：规范化后全等，或编辑距离 ≤ 1（容忍一次手滑）。
pub fn is_correct(answer: &str, target: &str) -> bool {
    let a = normalize_word(answer);
    let t = normalize_word(target);
    if a.is_empty() || t.is_empty() {
        return false;
    }
    // A typo in a short word is often another valid word (in/on, cat/cut).
    a == t
        || (t.chars().count() >= 5
            && a.chars().count().abs_diff(t.chars().count()) <= 1
            && edit_distance(&a, &t) <= 1)
}

/// 经典编辑距离（字符级 DP）。
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// 提示数 + 对错 → 复习质量（spec §12）。
/// 0 提示=Excellent，1=Good，2=Hard，3=Poor；答错/放弃=Forgotten。
pub fn quality_for(correct: bool, hints_used: u32) -> RecallQuality {
    if !correct {
        return RecallQuality::Forgotten;
    }
    match hints_used {
        0 => RecallQuality::Excellent,
        1 => RecallQuality::Good,
        2 => RecallQuality::Hard,
        _ => RecallQuality::Poor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_answers_are_all_hidden_and_short_words_require_exact_match() {
        assert_eq!(
            blank_word("A word is a word.", "word").as_deref(),
            Some("A ______ is a ______.")
        );
        assert!(!is_correct("on", "in"));
        assert!(!is_correct("cut", "cat"));
        assert!(is_correct("Subtl", "subtle"));
    }

    #[test]
    fn cloze_blanks_word_case_insensitively() {
        let out = blank_word(
            "She was extremely Meticulous when reviewing her code.",
            "meticulous",
        )
        .unwrap();
        assert_eq!(out, "She was extremely ______ when reviewing her code.");
    }

    #[test]
    fn cloze_respects_word_boundaries() {
        assert!(blank_word("The theme changed.", "he").is_none());
        assert_eq!(
            blank_word("He agreed.", "he").as_deref(),
            Some("______ agreed.")
        );
    }

    #[test]
    fn cloze_returns_none_when_word_absent() {
        assert!(blank_word("A completely unrelated sentence.", "meticulous").is_none());
    }

    #[test]
    fn prompt_kinds_follow_priority() {
        // 构造无匹配例句的词条：有英文释义 → Definition
        let entry = WordEntry {
            id: crate::domain::WordId(1),
            word: "zenith".into(),
            display: "zenith".into(),
            phonetic: None,
            frequency_rank: None,
            senses: vec![crate::dictionary::model::Sense {
                id: crate::domain::SenseId(1),
                pos: Some("n.".into()),
                english_definition: "the highest point".into(),
                chinese_definition: Some("顶点".into()),
                level: None,
                examples: vec![],
            }],
            collocations: vec![],
            synonyms: vec![],
            antonyms: vec![],
            word_family: vec![],
        };
        let p = build_prompt(&entry);
        assert_eq!(p.kind, PromptKind::Definition);
        assert_eq!(p.text, "the highest point");
        assert_eq!(p.sense_id, Some(crate::domain::SenseId(1)));
        assert_eq!(p.example_id, None);

        // 删掉英文释义 → 中文兜底 DefinitionZh
        let mut zh_only = entry.clone();
        zh_only.senses[0].english_definition = String::new();
        let p = build_prompt(&zh_only);
        assert_eq!(p.kind, PromptKind::DefinitionZh);
        assert_eq!(p.text, "顶点");

        // 词条带例句 → Cloze（词典来源）
        let mut with_example = entry.clone();
        with_example.senses[0].examples
            .push(crate::dictionary::model::Example {
                id: crate::domain::ExampleId(7),
                text: "The sun reached its zenith at noon.".into(),
                translation: None,
            });
        let p = build_prompt(&with_example);
        assert_eq!(p.kind, PromptKind::Cloze);
        assert_eq!(p.text, "The sun reached its ______ at noon.");
        assert_eq!(p.example_id, Some(crate::domain::ExampleId(7)));

        // 既无例句也无释义 → 空 prompt（调用方跳过该词）
        let mut empty = entry.clone();
        empty.senses[0].english_definition = String::new();
        empty.senses[0].chinese_definition = None;
        let p = build_prompt(&empty);
        assert!(p.text.trim().is_empty());
    }

    #[test]
    fn mask_hint_shapes() {
        assert_eq!(mask_hint("meticulous"), "m_________");
        assert_eq!(mask_hint("go"), "g_");
        assert_eq!(mask_hint(""), "");
    }

    #[test]
    fn hints_map_to_quality() {
        assert_eq!(quality_for(true, 0), RecallQuality::Excellent);
        assert_eq!(quality_for(true, 1), RecallQuality::Good);
        assert_eq!(quality_for(true, 2), RecallQuality::Hard);
        assert_eq!(quality_for(true, 3), RecallQuality::Poor);
        assert_eq!(quality_for(true, 7), RecallQuality::Poor);
        assert_eq!(quality_for(false, 0), RecallQuality::Forgotten);
    }

    #[test]
    fn answer_matching_normalizes_and_tolerates_typos() {
        assert!(is_correct("Meticulous", "meticulous"));
        assert!(is_correct(" meticulous ", "meticulous"));
        assert!(is_correct("meticulus", "meticulous")); // 编辑距离 1：容忍一次手滑
        assert!(!is_correct("Meticulosu", "meticulous")); // 相邻换位 = 距离 2
        assert!(!is_correct("meticulousss", "meticulous")); // 距离 2
        assert!(!is_correct("", "meticulous"));
        assert!(!is_correct("different", "meticulous"));
    }

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("a", ""), 1);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
    }
}
