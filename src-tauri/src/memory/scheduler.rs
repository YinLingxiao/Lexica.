//! 记忆调度器 v1：事件流 → 纯函数折算 → MemoryState 投影。
//!
//! 设计约束（设计文档 §6）：
//! - **核心不变式**：next_review 只在强化事件（成功复习 / Context 级理解）时延后；
//!   弱 visit 只能将其提前（min 规则）或不动。
//!   → “反复查词却没掌握”的词，永远不会因为“又看了一遍释义”而推迟复习。
//! - 参数全部集中在 Params（可解释、可调、可测）；换 FSRS = 新 fold + 全量重折。
//! - R(now) = θ^((now − last_reinforcement) / S)；status 按 S 阈值现算。

use chrono::{DateTime, Duration, Utc};

use super::model::{MemoryEvent, MemoryState, RecallQuality};
use crate::domain::{ComprehensionLevel, WordId};

pub trait MemoryScheduler {
    /// 事件流（时间序）→ 记忆状态投影。无事件 → None。
    fn fold(&self, word_id: WordId, events: &[MemoryEvent]) -> Option<MemoryState>;
    /// 当前可提取性 R ∈ (0, 1]。
    fn retrievability(&self, state: &MemoryState, now: DateTime<Utc>) -> f64;
}

#[derive(Debug, Clone, Default)]
pub struct V1Scheduler {
    pub params: Params,
}

#[derive(Debug, Clone)]
pub struct Params {
    /// θ：目标保持率。R 降到 θ 所需时间 = 一个 S。
    pub retention: f64,
    /// 初始稳定性（天），按首次 visit 的理解级别。
    pub s0_context: f64,
    pub s0_english: f64,
    pub s0_chinese: f64,
    pub s0_unknown: f64,
    /// 复习成功增益基数，按质量 [Poor, Hard, Good, Excellent]；实际增益 = gain × (base − D)。
    pub gain: [f64; 4],
    pub gain_difficulty_base: f64,
    /// Forgotten 处理。
    pub lapse_factor: f64,
    pub lapse_floor: f64,
    pub lapse_difficulty: f64,
    /// 例句级即懂 = 弱强化（有强提示的再现）。
    pub context_visit_gain: f64,
    /// 需中文 / 仍不懂的查词 = 弱 visit。
    pub weak_visit_factor: f64,
    pub weak_visit_difficulty: f64,
    /// 成功复习的难度缓解：D −= relief × (q − 1)。
    pub success_difficulty_relief: f64,
    pub difficulty_min: f64,
    pub difficulty_max: f64,
    pub difficulty0: f64,
    /// 弱 visit 把 due 提前的上限：due = min(旧due, at + max(floor, S_new × factor))。
    pub relearn_horizon_factor: f64,
    pub relearn_floor: f64,
    /// 优先级标记：weak_streak ≥ N，或 window 天内 ≥ M 次 visit。
    pub priority_weak_streak: u32,
    pub priority_visit_window_days: i64,
    pub priority_visit_count: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            retention: 0.90,
            s0_context: 2.0,
            s0_english: 1.0,
            s0_chinese: 0.5,
            s0_unknown: 0.25,
            gain: [1.3, 1.7, 2.3, 3.0],
            gain_difficulty_base: 1.1,
            lapse_factor: 0.3,
            lapse_floor: 0.25,
            lapse_difficulty: 0.15,
            context_visit_gain: 1.25,
            weak_visit_factor: 0.85,
            weak_visit_difficulty: 0.05,
            success_difficulty_relief: 0.03,
            difficulty_min: 0.05,
            difficulty_max: 0.95,
            difficulty0: 0.3,
            relearn_horizon_factor: 0.5,
            relearn_floor: 0.25,
            priority_weak_streak: 2,
            priority_visit_window_days: 14,
            priority_visit_count: 3,
        }
    }
}

fn days(d: f64) -> Duration {
    Duration::milliseconds((d * 86_400_000.0).round() as i64)
}

fn is_weak(level: ComprehensionLevel) -> bool {
    matches!(
        level,
        ComprehensionLevel::ChineseDefinition | ComprehensionLevel::Unknown
    )
}

/// 折算过程中的累积状态。
struct Core {
    strength_days: f64,
    difficulty: f64,
    review_count: u32,
    lapse_count: u32,
    visit_count: u32,
    weak_streak: u32,
    first_visit_at: DateTime<Utc>,
    last_visit_at: Option<DateTime<Utc>>,
    last_review_at: Option<DateTime<Utc>>,
    last_reinforcement_at: Option<DateTime<Utc>>,
    next_review_at: Option<DateTime<Utc>>,
}

impl MemoryScheduler for V1Scheduler {
    fn fold(&self, word_id: WordId, events: &[MemoryEvent]) -> Option<MemoryState> {
        let p = &self.params;
        let mut core: Option<Core> = None;
        let mut priority_visit_times: Vec<DateTime<Utc>> = Vec::new();

        for ev in events {
            match *ev {
                MemoryEvent::Visit { at, level } => {
                    if level == ComprehensionLevel::Context {
                        priority_visit_times.clear();
                    } else {
                        priority_visit_times.push(at);
                    }
                    match core.as_mut() {
                        None => {
                            let s0 = initial_strength(p, level);
                            core = Some(Core {
                                strength_days: s0,
                                difficulty: p.difficulty0,
                                review_count: 0,
                                lapse_count: 0,
                                visit_count: 1,
                                weak_streak: u32::from(is_weak(level)),
                                first_visit_at: at,
                                last_visit_at: Some(at),
                                last_review_at: None,
                                last_reinforcement_at: Some(at),
                                next_review_at: Some(at + days(s0)),
                            });
                        }
                        Some(c) => self.apply_visit(c, at, level),
                    }
                }
                MemoryEvent::ReviewDone { at, quality } => {
                    if quality != RecallQuality::Forgotten {
                        priority_visit_times.clear();
                    }
                    // 防御：复习必先有 visit（服务层保证）；异常数据按 unknown 起点处理。
                    if let Some(c) = core.as_mut() {
                        self.apply_review(c, at, quality);
                    } else {
                        let mut c = Core {
                            strength_days: p.s0_unknown,
                            difficulty: p.difficulty0,
                            review_count: 0,
                            lapse_count: 0,
                            visit_count: 0,
                            weak_streak: 0,
                            first_visit_at: at,
                            last_visit_at: None,
                            last_review_at: None,
                            last_reinforcement_at: Some(at),
                            next_review_at: Some(at + days(p.s0_unknown)),
                        };
                        self.apply_review(&mut c, at, quality);
                        core = Some(c);
                    }
                }
            }
        }

        let c = core?;
        let high_priority = c.weak_streak >= p.priority_weak_streak
            || visits_in_window(
                &priority_visit_times,
                Duration::days(p.priority_visit_window_days),
                p.priority_visit_count,
            );
        Some(MemoryState {
            word_id,
            strength_days: c.strength_days,
            difficulty: c.difficulty,
            review_count: c.review_count,
            lapse_count: c.lapse_count,
            visit_count: c.visit_count,
            weak_streak: c.weak_streak,
            high_priority,
            first_visit_at: c.first_visit_at,
            last_visit_at: c.last_visit_at,
            last_review_at: c.last_review_at,
            last_reinforcement_at: c.last_reinforcement_at,
            next_review_at: c.next_review_at,
        })
    }

    fn retrievability(&self, state: &MemoryState, now: DateTime<Utc>) -> f64 {
        match state.last_reinforcement_at {
            None => 0.0,
            Some(anchor) => {
                let dt_days = (now - anchor).num_milliseconds() as f64 / 86_400_000.0;
                if dt_days <= 0.0 {
                    return 1.0;
                }
                self.params
                    .retention
                    .powf(dt_days / state.strength_days.max(f64::MIN_POSITIVE))
            }
        }
    }
}

impl V1Scheduler {
    fn apply_visit(&self, c: &mut Core, at: DateTime<Utc>, level: ComprehensionLevel) {
        let p = &self.params;
        c.visit_count += 1;
        c.last_visit_at = Some(at);
        match level {
            // 强化：小步增长 + due 正常延后
            ComprehensionLevel::Context => {
                c.strength_days *= p.context_visit_gain;
                c.next_review_at = Some(at + days(c.strength_days));
                c.last_reinforcement_at = Some(at);
                c.weak_streak = 0;
            }
            // 中性：有暴露但未掌握，不给信号（只计数）
            ComprehensionLevel::EnglishDefinition => {}
            // 弱 visit：收缩 S、due 只能提前（★不变式）
            ComprehensionLevel::ChineseDefinition | ComprehensionLevel::Unknown => {
                c.strength_days = (c.strength_days * p.weak_visit_factor).max(p.lapse_floor);
                c.difficulty = (c.difficulty + p.weak_visit_difficulty).min(p.difficulty_max);
                c.weak_streak += 1;
                let horizon = (c.strength_days * p.relearn_horizon_factor).max(p.relearn_floor);
                let candidate = at + days(horizon);
                c.next_review_at = Some(match c.next_review_at {
                    Some(nr) => nr.min(candidate),
                    None => candidate,
                });
            }
        }
    }

    fn apply_review(&self, c: &mut Core, at: DateTime<Utc>, quality: RecallQuality) {
        let p = &self.params;
        c.review_count += 1;
        c.last_review_at = Some(at);
        c.last_reinforcement_at = Some(at);
        c.weak_streak = 0;
        match quality {
            RecallQuality::Forgotten => {
                c.strength_days = (c.strength_days * p.lapse_factor).max(p.lapse_floor);
                c.difficulty = (c.difficulty + p.lapse_difficulty).min(p.difficulty_max);
                c.lapse_count += 1;
            }
            other => {
                let qi = other.index(); // 1..=4
                let gain = p.gain[qi - 1] * (p.gain_difficulty_base - c.difficulty);
                c.strength_days = (c.strength_days * gain).max(p.lapse_floor);
                c.difficulty = (c.difficulty - p.success_difficulty_relief * (qi as f64 - 1.0))
                    .max(p.difficulty_min);
            }
        }
        c.next_review_at = Some(at + days(c.strength_days));
    }
}

fn initial_strength(p: &Params, level: ComprehensionLevel) -> f64 {
    match level {
        ComprehensionLevel::Context => p.s0_context,
        ComprehensionLevel::EnglishDefinition => p.s0_english,
        ComprehensionLevel::ChineseDefinition => p.s0_chinese,
        ComprehensionLevel::Unknown => p.s0_unknown,
    }
}

/// 滑动窗口：是否存在 window 内 ≥ need 次 visit（visit_times 已时间序）。
fn visits_in_window(times: &[DateTime<Utc>], window: Duration, need: usize) -> bool {
    if times.len() < need || need == 0 {
        return false;
    }
    for i in 0..times.len() {
        let mut count = 1; // 自身
        for t in &times[i + 1..] {
            if *t - times[i] <= window {
                count += 1;
                if count >= need {
                    return true;
                }
            } else {
                break;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::MemoryStatus;
    use chrono::TimeZone;

    fn ts(d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 1, 12, 0, 0).unwrap() + Duration::days(i64::from(d) - 1)
    }

    fn sched() -> V1Scheduler {
        V1Scheduler::default()
    }

    fn visit(d: u32, level: ComprehensionLevel) -> MemoryEvent {
        MemoryEvent::Visit { at: ts(d), level }
    }

    fn review(d: u32, q: RecallQuality) -> MemoryEvent {
        MemoryEvent::ReviewDone {
            at: ts(d),
            quality: q,
        }
    }

    fn fold(events: &[MemoryEvent]) -> MemoryState {
        sched().fold(WordId(1), events).unwrap()
    }

    // ── 初始稳定性 ──────────────────────────────────────────

    #[test]
    fn s0_by_first_level() {
        for (level, expected) in [
            (ComprehensionLevel::Context, 2.0),
            (ComprehensionLevel::EnglishDefinition, 1.0),
            (ComprehensionLevel::ChineseDefinition, 0.5),
            (ComprehensionLevel::Unknown, 0.25),
        ] {
            let s = fold(&[visit(1, level)]);
            assert!((s.strength_days - expected).abs() < 1e-9, "{level:?}");
            assert_eq!(s.next_review_at, Some(ts(1) + days(expected)));
            assert_eq!(s.visit_count, 1);
        }
    }

    // ── 遗忘曲线 ────────────────────────────────────────────

    #[test]
    fn retrievability_decays_over_time() {
        let s = sched();
        let state = fold(&[visit(1, ComprehensionLevel::EnglishDefinition)]);
        let anchor = ts(1);

        assert!((s.retrievability(&state, anchor) - 1.0).abs() < 1e-9);
        // Δt = S → R = θ
        let at_s = anchor + days(state.strength_days);
        assert!((s.retrievability(&state, at_s) - 0.90).abs() < 1e-6);
        // Δt = 2S → R = θ²
        let at_2s = anchor + days(2.0 * state.strength_days);
        assert!((s.retrievability(&state, at_2s) - 0.81).abs() < 1e-6);
        // 单调下降
        let r1 = s.retrievability(&state, anchor + days(0.3));
        let r2 = s.retrievability(&state, anchor + days(0.6));
        assert!(r1 > r2);
    }

    // ── 复习效果 ────────────────────────────────────────────

    #[test]
    fn successful_recall_extends_and_grows_interval() {
        let mut events = vec![visit(1, ComprehensionLevel::ChineseDefinition)];
        let mut prev_due = fold(&events).next_review_at.unwrap();
        let mut prev_strength = fold(&events).strength_days;

        for day in [2u32, 3, 4, 5] {
            events.push(review(day, RecallQuality::Good));
            let s = fold(&events);
            assert!(s.strength_days > prev_strength, "strength 应单调增长");
            assert!(s.next_review_at.unwrap() > prev_due, "due 应持续延后");
            prev_due = s.next_review_at.unwrap();
            prev_strength = s.strength_days;
        }
    }

    #[test]
    fn forgotten_shrinks_interval() {
        let events = vec![
            visit(1, ComprehensionLevel::ChineseDefinition),
            review(2, RecallQuality::Good),
        ];
        let before = fold(&events);
        let with_lapse = fold(&[events[0], events[1], review(3, RecallQuality::Forgotten)]);
        assert!(with_lapse.strength_days < before.strength_days);
        let before_interval = before.next_review_at.unwrap() - ts(2);
        let lapse_interval = with_lapse.next_review_at.unwrap() - ts(3);
        assert!(lapse_interval < before_interval);
        assert_eq!(with_lapse.lapse_count, 1);
    }

    #[test]
    fn forgotten_then_good_recovers() {
        let s = fold(&[
            visit(1, ComprehensionLevel::ChineseDefinition),
            review(2, RecallQuality::Good),
            review(3, RecallQuality::Forgotten),
            review(4, RecallQuality::Good),
        ]);
        // 复原后仍高于 lapse 下限，且 review_count 齐全
        assert!(s.strength_days > 0.25);
        assert_eq!(s.review_count, 3);
        assert_eq!(s.lapse_count, 1);
    }

    // ── 核心不变式 ──────────────────────────────────────────

    #[test]
    fn weak_or_english_visit_never_extends_due() {
        // 序列：查词(chinese) → 复习(Good) → 各种弱/中性 visit，due 不得晚于 visit 前
        let base = vec![
            visit(1, ComprehensionLevel::ChineseDefinition),
            review(3, RecallQuality::Good),
        ];
        let due_before = fold(&base).next_review_at.unwrap();

        // english visit：中性，due 不变
        let after_english = fold(&[
            base[0],
            base[1],
            visit(4, ComprehensionLevel::EnglishDefinition),
        ]);
        assert_eq!(after_english.next_review_at, Some(due_before));

        // chinese visit：只能提前
        let after_chinese = fold(&[
            base[0],
            base[1],
            visit(4, ComprehensionLevel::ChineseDefinition),
        ]);
        assert!(after_chinese.next_review_at.unwrap() <= due_before);

        // 连续多次弱 visit 依然不延后（acquire 场景）
        let mut events = base.clone();
        for day in [4u32, 6, 8, 10] {
            events.push(visit(day, ComprehensionLevel::ChineseDefinition));
        }
        let after_many = fold(&events);
        assert!(after_many.next_review_at.unwrap() <= due_before);
    }

    // ── 优先级标记（spec §10：反复查词） ─────────────────────

    #[test]
    fn repeated_lookups_flag_high_priority() {
        // acquire 案例：09/01、09/03、09/05、09/09 四次查词
        let events = [
            visit(1, ComprehensionLevel::ChineseDefinition),
            visit(3, ComprehensionLevel::EnglishDefinition),
            visit(5, ComprehensionLevel::ChineseDefinition),
            visit(9, ComprehensionLevel::EnglishDefinition),
        ];
        let s = fold(&events);
        assert!(s.high_priority, "14 天内 ≥3 次查词应标记高优先级");
        assert!(s.weak_streak >= 2 || s.visit_count >= 3);

        let mastered = fold(&[
            events[0],
            events[1],
            events[2],
            events[3],
            review(10, RecallQuality::Good),
        ]);
        assert!(!mastered.high_priority, "成功复习应清除旧的查词突发优先级");

        // 对照：单次查词不标记
        let single = fold(&[visit(1, ComprehensionLevel::ChineseDefinition)]);
        assert!(!single.high_priority);
    }

    #[test]
    fn weak_streak_flag_and_reset() {
        // 连续两次弱 visit → 标记
        let s = fold(&[
            visit(1, ComprehensionLevel::ChineseDefinition),
            visit(3, ComprehensionLevel::Unknown),
        ]);
        assert_eq!(s.weak_streak, 2);
        assert!(s.high_priority);

        // context 理解清零
        let s2 = fold(&[
            visit(1, ComprehensionLevel::ChineseDefinition),
            visit(3, ComprehensionLevel::Unknown),
            visit(12, ComprehensionLevel::Context),
        ]);
        assert_eq!(s2.weak_streak, 0);
    }

    // ── 轨迹识别（spec §8：掌握正在增强） ────────────────────

    #[test]
    fn trajectory_chinese_english_context_improves() {
        // 09/01 需中文 → 09/03 需英英 → 09/12 例句即懂
        let s = fold(&[
            visit(1, ComprehensionLevel::ChineseDefinition),
            visit(3, ComprehensionLevel::EnglishDefinition),
            visit(12, ComprehensionLevel::Context),
        ]);
        let first = fold(&[visit(1, ComprehensionLevel::ChineseDefinition)]);

        assert!(s.strength_days > first.strength_days, "S 应增长");
        assert_eq!(s.weak_streak, 0, "context 理解清零弱连击");
        assert_eq!(s.last_reinforcement_at, Some(ts(12)));
        assert!(s.next_review_at.unwrap() > ts(12), "due 从最后一次强化起算");
        assert_eq!(s.visit_count, 3);
    }

    // ── 纯度 ────────────────────────────────────────────────

    #[test]
    fn fold_is_deterministic() {
        let events = [
            visit(1, ComprehensionLevel::ChineseDefinition),
            review(3, RecallQuality::Good),
            visit(5, ComprehensionLevel::EnglishDefinition),
            review(7, RecallQuality::Hard),
        ];
        let a = fold(&events);
        let b = fold(&events);
        assert_eq!(a, b);
    }

    #[test]
    fn fold_empty_returns_none() {
        assert!(sched().fold(WordId(1), &[]).is_none());
    }

    #[test]
    fn status_buckets_by_strength() {
        let learning = fold(&[visit(1, ComprehensionLevel::ChineseDefinition)]);
        assert_eq!(learning.status(), MemoryStatus::Learning);

        let mut events = vec![visit(1, ComprehensionLevel::Context)];
        while fold(&events).strength_days < 4.0 {
            events.push(review(15, RecallQuality::Good));
        }
        assert_eq!(fold(&events).status(), MemoryStatus::Familiar);

        while fold(&events).strength_days < 21.0 {
            events.push(review(40, RecallQuality::Good));
        }
        assert_eq!(fold(&events).status(), MemoryStatus::Stable);
    }
}
