//! 离线词根词缀拆解与形近词（编辑距离）算法。
//!
//! 当 `word_enrichments.roots_json` 为空时，使用内置词根词缀规则表自动拆解；
//! 形近词通过首字母索引范围 + 词长 ±2 过滤 + Levenshtein 距离 1..=2 选出。

use super::model::RootPart;

/// 常见英语前缀规则表（按长度降序排列以优先匹配长前缀）。
pub const PREFIX_RULES: &[(&str, &str)] = &[
    ("counter", "反对，相对"),
    ("circum", "环绕，周围"),
    ("contra", "反对，相反"),
    ("micro", "微小，显微"),
    ("inter", "在…之间，相互"),
    ("trans", "跨越，转变"),
    ("super", "超级，在…之上"),
    ("under", "在…之下，不足"),
    ("multi", "多，多种"),
    ("hyper", "超过，过度"),
    ("ultra", "极端，超出"),
    ("extra", "额外，以外"),
    ("intra", "在内，内部"),
    ("intro", "向内，进入"),
    ("retro", "向后，追溯"),
    ("over", "过度，在…之上"),
    ("anti", "反对，抵抗"),
    ("auto", "自己，自动"),
    ("fore", "预先，在前"),
    ("post", "在…之后"),
    ("tele", "远距离"),
    ("mono", "单一，独"),
    ("semi", "半，部分"),
    ("pre", "在…之前，预先"),
    ("pro", "向前，赞成"),
    ("sub", "在…下，次"),
    ("dis", "否定，分开，去除"),
    ("mis", "错误，不当"),
    ("non", "非，不"),
    ("out", "超过，向外"),
    ("per", "贯穿，完全"),
    ("con", "共同，加强"),
    ("com", "共同，加强"),
    ("col", "共同，一起"),
    ("cor", "共同，加强"),
    ("syn", "共同，同步"),
    ("sym", "共同，相同"),
    ("uni", "单一，统一"),
    ("tri", "三"),
    ("re", "再次，向后，返回"),
    ("de", "向下，去除，否定"),
    ("ex", "向外，前任"),
    ("in", "向内；不，无"),
    ("im", "向内；不，无"),
    ("ir", "不，无"),
    ("il", "不，无"),
    ("un", "不，无，相反动作"),
    ("en", "使成为，进入"),
    ("em", "使进入，置于"),
    ("ab", "脱离，偏离"),
    ("ad", "朝向，加强"),
    ("bi", "二，双"),
    ("co", "共同，一起"),
];

/// 常见英语词根规则表（按长度降序排列）。
pub const ROOT_RULES: &[(&str, &str)] = &[
    ("surrect", "上升，升起"),
    ("deterior", "恶化，变坏"),
    ("spont", "自愿，自发"),
    ("scrut", "搜寻，详查"),
    ("pragm", "行动，事务"),
    ("stren", "活跃，用力"),
    ("vigil", "清醒，警觉"),
    ("ambig", "驱使，徘徊"),
    ("mitig", "缓和，使温和"),
    ("labor", "劳动，精心制作"),
    ("plaus", "鼓掌，可信"),
    ("plaud", "鼓掌，赞许"),
    ("haust", "汲取，抽干"),
    ("struct", "建造，构建"),
    ("script", "写，记录"),
    ("spect", "看，观察"),
    ("tract", "拉，抽，吸引"),
    ("press", "压，按"),
    ("ceive", "拿，抓，接纳"),
    ("quir", "寻求，获得"),
    ("quis", "寻求，获得"),
    ("evit", "避免"),
    ("luct", "挣扎，抗争"),
    ("fund", "底部，基础"),
    ("spic", "看，观察"),
    ("spec", "看，观察"),
    ("feas", "做，实行"),
    ("vers", "旋转，转向"),
    ("vert", "旋转，转向"),
    ("skep", "观察，审视"),
    ("scop", "观察，审视"),
    ("tent", "伸展，尝试"),
    ("tend", "伸展，趋向"),
    ("tens", "伸展，张力"),
    ("tang", "接触，触摸"),
    ("tact", "接触，触摸"),
    ("triv", "三岔路，琐碎"),
    ("surg", "上升，升起"),
    ("sure", "确信，安全"),
    ("dict", "说，断言"),
    ("scrib", "写，记录"),
    ("port", "拿，运送"),
    ("form", "形状，形式"),
    ("ject", "投掷，扔"),
    ("ceed", "走，行进，让步"),
    ("cess", "走，行进，让步"),
    ("cept", "拿，抓，容纳"),
    ("tain", "握，持，保持"),
    ("sist", "站立，坚持"),
    ("stat", "站立，状态"),
    ("grad", "步，走，等级"),
    ("gress", "步，走，进展"),
    ("lect", "选择，阅读"),
    ("pend", "悬挂，称重，花费"),
    ("pens", "悬挂，衡量，花费"),
    ("flux", "流动"),
    ("junct", "连接"),
    ("rupt", "破裂，打断"),
    ("scend", "攀爬，上升"),
    ("sect", "切割，部分"),
    ("sens", "感觉，意识"),
    ("sent", "感觉，同意"),
    ("serv", "保持，服务"),
    ("sign", "记号，签署"),
    ("solv", "松开，解开"),
    ("spir", "呼吸，精神"),
    ("strict", "拉紧，严格"),
    ("term", "边界，终点"),
    ("tort", "扭曲"),
    ("vent", "来，出现"),
    ("volv", "滚动，转动"),
    ("chron", "时间"),
    ("graph", "写，画，记录"),
    ("gram", "写，记录"),
    ("phon", "声音"),
    ("path", "感情，痛苦，疾病"),
    ("phil", "爱，偏好"),
    ("pel", "驱动，推动"),
    ("puls", "驱动，搏动"),
    ("sil", "跳，弹回"),
    ("her", "粘附，依附"),
    ("hes", "粘附，连贯"),
    ("min", "突出；缩小，削弱"),
    ("rig", "僵直，严格"),
    ("tric", "纠葛，复杂"),
    ("nov", "新"),
    ("vas", "走，蔓延"),
    ("vad", "走，推进"),
    ("val", "强壮，有效，盛行"),
    ("fut", "击打，驳倒"),
    ("ted", "厌倦，疲乏"),
    ("ver", "真实，核实"),
    ("war", "留意，看守"),
    ("duc", "引导，带来"),
    ("cap", "拿，抓"),
    ("ten", "握，持"),
    ("fer", "带，拿，承受"),
    ("pos", "放置"),
    ("pon", "放置"),
    ("vid", "看"),
    ("vis", "看"),
    ("aud", "听"),
    ("log", "说话，理性，学科"),
    ("bio", "生命"),
    ("geo", "地球，土地"),
    ("fin", "界限，结束"),
    ("flu", "流动"),
    ("gen", "出生，产生，种类"),
    ("vac", "空"),
    ("ven", "来"),
    ("viv", "活，生命"),
    ("vit", "活，生命"),
    ("voc", "呼喊，声音"),
];

/// 常见英语后缀规则表（按长度降序排列）。
pub const SUFFIX_RULES: &[(&str, &str)] = &[
    ("ization", "构成名词，表示‘…化’或过程"),
    ("ation", "构成名词，表示行为、过程或结果"),
    ("ition", "构成名词，表示行为或状态"),
    ("ious", "构成形容词，表示‘充满…的’或‘具有…性质的’"),
    ("uous", "构成形容词，表示‘多…的’或‘有…性质的’"),
    ("able", "构成形容词，表示‘可…的’或‘能…的’"),
    ("ible", "构成形容词，表示‘可…的’或‘具备…性质的’"),
    ("ment", "构成名词，表示行为、结果或状态"),
    ("ness", "构成名词，表示性质或状态"),
    ("less", "构成形容词，表示‘无…的’"),
    ("ship", "构成名词，表示身份、关系或状态"),
    ("ward", "构成形容词或副词，表示方向"),
    ("wise", "构成副词，表示方向或方式"),
    ("ence", "构成名词，表示性质、状态或行为"),
    ("ance", "构成名词，表示性质、状态或行为"),
    ("ient", "构成形容词，表示‘处于…状态的’"),
    ("tive", "构成形容词，表示‘有…倾向的’"),
    ("ion", "构成名词，表示行为、过程或结果"),
    ("ity", "构成名词，表示性质或状态"),
    ("ive", "构成形容词，表示‘有…性质的’或‘倾向于…的’"),
    ("ous", "构成形容词，表示‘充满…的’"),
    ("ful", "构成形容词，表示‘充满…的’"),
    ("ize", "构成动词，表示‘使…化’"),
    ("ise", "构成动词，表示‘使…化’"),
    ("ify", "构成动词，表示‘使成为…’"),
    ("ate", "构成动词或形容词，表示‘做…’或‘具有…特征的’"),
    ("ent", "构成形容词或名词，表示‘…的’或‘施动者’"),
    ("ant", "构成形容词或名词，表示‘…的’或‘施动者’"),
    ("ory", "构成名词或形容词，表示‘…的地方’或‘与…相关的事物’"),
    ("ary", "构成形容词或名词，表示‘与…有关的’或‘场所’"),
    ("ery", "构成名词，表示场所、行为或技艺"),
    ("ic", "构成形容词，表示‘与…有关的’"),
    ("al", "构成形容词或名词，表示‘属于…的’或行为"),
    ("ly", "构成副词或形容词，表示‘…地’或‘如…般的’"),
    ("er", "构成名词，表示‘从事…的人或物’"),
    ("or", "构成名词，表示‘施动者或器物’"),
    ("ty", "构成名词，表示性质或状态"),
    ("y", "构成形容词或名词，表示‘充满…的’或状态"),
];

/// 内置词根、前缀、后缀规则总数（不少于 60 条）。
pub fn rule_count() -> usize {
    PREFIX_RULES.len() + ROOT_RULES.len() + SUFFIX_RULES.len()
}

/// 离线词根词缀拆解：当 `word_enrichments.roots_json` 为空时自动拆解单词。
pub fn decompose_roots(word: &str) -> Vec<RootPart> {
    let w = word.trim().to_ascii_lowercase();
    if w.is_empty() {
        return Vec::new();
    }

    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut end = w.len();

    // 1. 匹配前缀（确保剩余词干至少 3 个字符）
    for &(prefix, meaning) in PREFIX_RULES {
        if w.starts_with(prefix) && w.len() >= prefix.len() + 3 {
            parts.push(RootPart {
                part: format!("{prefix}-"),
                meaning: meaning.to_string(),
            });
            start = prefix.len();
            break;
        }
    }

    // 2. 匹配后缀（确保剩余词干至少 2 个字符）
    let mut matched_suffix: Option<RootPart> = None;
    let remaining_for_suffix = &w[start..];
    for &(suffix, meaning) in SUFFIX_RULES {
        if remaining_for_suffix.ends_with(suffix) && remaining_for_suffix.len() >= suffix.len() + 2 {
            matched_suffix = Some(RootPart {
                part: format!("-{suffix}"),
                meaning: meaning.to_string(),
            });
            end = w.len() - suffix.len();
            break;
        }
    }

    // 3. 在词干（或全词）中匹配词根
    let stem = &w[start..end];
    let mut found_root = false;
    for &(root, meaning) in ROOT_RULES {
        if stem.contains(root) {
            parts.push(RootPart {
                part: root.to_string(),
                meaning: meaning.to_string(),
            });
            found_root = true;
            break;
        }
    }

    if !found_root {
        for &(root, meaning) in ROOT_RULES {
            if w.contains(root) {
                parts.push(RootPart {
                    part: root.to_string(),
                    meaning: meaning.to_string(),
                });
                found_root = true;
                break;
            }
        }
    }

    if !found_root && !stem.is_empty() && (parts.is_empty() || matched_suffix.is_some()) {
        parts.push(RootPart {
            part: stem.to_string(),
            meaning: "核心词干".to_string(),
        });
    }

    if let Some(suffix_part) = matched_suffix {
        parts.push(suffix_part);
    }

    parts
}

/// 计算两个字符串的字符级 Levenshtein 编辑距离。
pub fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for (i, &ca) in a_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b_chars.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// 解析 `word_family` 条目，如 `"microsurgery n. 显微外科"` 或 `"meticulously adv."`。
/// 返回 `(word, pos, inline_gloss)`。
pub fn parse_word_family_entry(raw: &str) -> (String, Option<String>, Option<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (String::new(), None, None);
    }

    let mut iter = trimmed.split_whitespace();
    let Some(word) = iter.next() else {
        return (String::new(), None, None);
    };
    let rest_after_word = trimmed[word.len()..].trim();
    if rest_after_word.is_empty() {
        return (word.to_string(), None, None);
    }

    if let Some(second) = iter.next() {
        if is_pos_token(second) {
            let after_pos = rest_after_word[second.len()..].trim();
            let gloss = (!after_pos.is_empty()).then(|| after_pos.to_string());
            return (word.to_string(), Some(second.to_string()), gloss);
        }
    }

    (word.to_string(), None, Some(rest_after_word.to_string()))
}

pub fn is_pos_token(token: &str) -> bool {
    matches!(
        token,
        "n." | "v."
            | "vt."
            | "vi."
            | "adj."
            | "adv."
            | "prep."
            | "conj."
            | "pron."
            | "art."
            | "num."
            | "int."
            | "a."
            | "s."
            | "noun"
            | "verb"
            | "adjective"
            | "adverb"
    )
}

/// 从释义文本中提取词性并清洗重复的词性前缀，生成简明中文释义。
pub fn extract_pos_and_clean_gloss(
    pos: Option<String>,
    zh: Option<&str>,
    en: Option<&str>,
) -> (Option<String>, String) {
    let raw_gloss = zh
        .filter(|s| !s.trim().is_empty())
        .or_else(|| en.filter(|s| !s.trim().is_empty()))
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .trim();

    let mut final_pos = pos.filter(|p| !p.trim().is_empty());
    let mut clean_gloss = raw_gloss.to_string();

    for prefix in [
        "n.", "vt.", "vi.", "v.", "adj.", "adv.", "prep.", "conj.", "pron.", "art.", "num.",
        "int.", "a.",
    ] {
        if let Some(rest) = raw_gloss.strip_prefix(prefix) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                if final_pos.is_none() {
                    let mapped = if prefix == "a." { "adj." } else { prefix };
                    final_pos = Some(mapped.to_string());
                }
                let stripped = rest.trim();
                if !stripped.is_empty() {
                    clean_gloss = stripped.to_string();
                }
                break;
            }
        }
    }

    (final_pos, clean_gloss)
}

/// 种子词关联词（派生词/同义词/反义词）在未导入全量 ECDICT 时的兜底词性与中文简明释义。
pub fn fallback_related_gloss(word: &str) -> Option<(&'static str, &'static str)> {
    match word {
        // surgery 及其关联词
        "microsurgery" => Some(("n.", "显微外科")),
        "surgeon" => Some(("n.", "外科医生")),
        "surgical" => Some(("adj.", "外科的；手术的")),
        "surgically" => Some(("adv.", "外科手术地")),
        "operation" => Some(("n.", "外科手术；操作")),
        "procedure" => Some(("n.", "医疗程序；步骤")),
        "surety" => Some(("n.", "保证金，担保人")),
        // religion 及其关联词
        "religionist" => Some(("n.", "宗教信徒；笃信宗教者")),
        "irreligion" => Some(("n.", "无宗教信仰；对宗教漠不关心")),
        "region" => Some(("n.", "区域，地区，地方，部位")),
        "relation" => Some(("n.", "关系，关联，亲属")),
        "revision" => Some(("n.", "修改，修订，复习")),
        // meticulous 及其关联词
        "meticulously" => Some(("adv.", "一丝不苟地；细致地")),
        "meticulousness" => Some(("n.", "一丝不苟；极其仔细")),
        "careful" => Some(("adj.", "仔细的；小心的")),
        "painstaking" => Some(("adj.", "煞费苦心的；刻苦的")),
        "thorough" => Some(("adj.", "彻底的；详尽的")),
        "scrupulous" => Some(("adj.", "严谨的；一丝不苟的")),
        "careless" => Some(("adj.", "粗心的；疏忽的")),
        "sloppy" => Some(("adj.", "马虎的；草率的")),
        "slapdash" => Some(("adj.", "仓促马虎的；敷衍的")),
        // elaborate 及其关联词
        "elaborately" => Some(("adv.", "精心地；详尽地")),
        "elaboration" => Some(("n.", "详尽阐述；精心制作")),
        "detailed" => Some(("adj.", "详细的；细致的")),
        "ornate" => Some(("adj.", "华丽的；装饰繁复的")),
        "simple" => Some(("adj.", "简单的；朴素的")),
        "plain" => Some(("adj.", "清晰的；朴素的")),
        "modest" => Some(("adj.", "适度的；谦虚的")),
        // subtle 及其关联词
        "subtly" => Some(("adv.", "微妙地；巧妙地")),
        "subtlety" => Some(("n.", "微妙；敏锐")),
        "slight" => Some(("adj.", "轻微的；细小的")),
        "faint" => Some(("adj.", "微弱的；模糊的")),
        "understated" => Some(("adj.", "低调的；含蓄的")),
        "obvious" => Some(("adj.", "明显的；显而易见的")),
        "blatant" => Some(("adj.", "公然的；露骨的")),
        "pronounced" => Some(("adj.", "显著的；明显的")),
        // acquire 及其关联词
        "acquisition" => Some(("n.", "获得；收购")),
        "acquired" => Some(("adj.", "后天习得的；已获得的")),
        "obtain" => Some(("v.", "获得；取得")),
        "gain" => Some(("v.", "获得；增加")),
        "attain" => Some(("v.", "达到；获得")),
        "lose" => Some(("v.", "失去；丢失")),
        "forfeit" => Some(("v.", "丧失；被没收")),
        "relinquish" => Some(("v.", "放弃；让出")),
        // inevitable 及其关联词
        "inevitably" => Some(("adv.", "不可避免地；必然地")),
        "inevitability" => Some(("n.", "必然性；不可避免")),
        "unavoidable" => Some(("adj.", "不可避免的")),
        "certain" => Some(("adj.", "确定的；必然的")),
        "inescapable" => Some(("adj.", "无法逃避的")),
        "avoidable" => Some(("adj.", "可避免的")),
        "preventable" => Some(("adj.", "可预防的")),
        "uncertain" => Some(("adj.", "不确定的")),
        // 其他高频种子派生词
        "reluctantly" => Some(("adv.", "不情愿地；勉强地")),
        "reluctance" => Some(("n.", "不情愿；勉强")),
        "compel" => Some(("v.", "强迫；迫使")),
        "compellingly" => Some(("adv.", "令人信服地")),
        "profoundly" => Some(("adv.", "深刻地；极大地")),
        "profundity" => Some(("n.", "深刻；深奥")),
        "ambiguity" => Some(("n.", "模棱两可；歧义")),
        "ambiguously" => Some(("adv.", "含糊地；模棱两可地")),
        "resilience" => Some(("n.", "韧性；恢复力")),
        "pragmatism" => Some(("n.", "实用主义；务实")),
        "pragmatically" => Some(("adv.", "务实地；讲求实际地")),
        "scrutiny" => Some(("n.", "仔细审查；监督")),
        "tentatively" => Some(("adv.", "试探性地；暂定地")),
        "mitigation" => Some(("n.", "减轻；缓和")),
        "mitigating" => Some(("adj.", "减轻的；缓和的")),
        "conspicuously" => Some(("adv.", "显眼地；引人注目地")),
        "deterioration" => Some(("n.", "恶化；退化")),
        "feasibility" => Some(("n.", "可行性")),
        "inherently" => Some(("adv.", "固等地；本质上")),
        "versatility" => Some(("n.", "多功能性；多才多艺")),
        "skepticism" => Some(("n.", "怀疑态度；怀疑论")),
        "skeptic" => Some(("n.", "怀疑论者")),
        "skeptically" => Some(("adv.", "怀疑地")),
        "rigor" => Some(("n.", "严格；缜密")),
        "rigorously" => Some(("adv.", "严格地；缜密地")),
        "coherence" => Some(("n.", "连贯性；条理")),
        "coherently" => Some(("adv.", "连贯地；有条理地")),
        "cohere" => Some(("v.", "连贯；凝聚")),
        "diminishing" => Some(("adj.", "逐渐减少的")),
        "exhaustively" => Some(("adv.", "详尽无遗地")),
        "hindrance" => Some(("n.", "阻碍；妨碍")),
        "imperatively" => Some(("adv.", "迫切地；必要地")),
        "intricately" => Some(("adv.", "错综复杂地")),
        "intricacy" => Some(("n.", "错综复杂")),
        "lingering" => Some(("adj.", "拖延的；挥之不去的")),
        "obsolescence" => Some(("n.", "过时；淘汰")),
        "pervasively" => Some(("adv.", "普遍地；无处不在地")),
        "plausibility" => Some(("n.", "貌似可信；合理性")),
        "plausibly" => Some(("adv.", "貌似有理地")),
        "implausible" => Some(("adj.", "难以置信的；不像真实的")),
        "prevalence" => Some(("n.", "普遍；流行")),
        "refutation" => Some(("n.", "反驳；驳斥")),
        "spontaneously" => Some(("adv.", "自发地；自然地")),
        "spontaneity" => Some(("n.", "自发性；自然流露")),
        "strenuously" => Some(("adv.", "奋力地；剧烈地")),
        "tangibly" => Some(("adv.", "切实地；有形地")),
        "intangible" => Some(("adj.", "无形的；难以捉摸的")),
        "tediously" => Some(("adv.", "冗长乏味地")),
        "tedium" => Some(("n.", "单调乏味；厌倦")),
        "thriving" => Some(("adj.", "繁荣的；兴旺的")),
        "trivially" => Some(("adv.", "琐碎地；不重要地")),
        "triviality" => Some(("n.", "琐事；肤浅")),
        "utterly" => Some(("adv.", "完全地；彻底地")),
        "utterance" => Some(("n.", "话语；表达")),
        "verification" => Some(("n.", "核实；验证")),
        "verifiable" => Some(("adj.", "可核实的；可验证的")),
        "vigilantly" => Some(("adv.", "警惕地；警觉地")),
        "vigilance" => Some(("n.", "警惕；警觉")),
        "warily" => Some(("adv.", "谨慎地；提防地")),
        "wariness" => Some(("n.", "谨慎；戒心")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_table_has_at_least_60_entries() {
        assert!(
            rule_count() >= 60,
            "expected at least 60 affix/root rules, got {}",
            rule_count()
        );
    }

    #[test]
    fn decompose_roots_splits_known_affixes_and_roots() {
        let parts = decompose_roots("inevitable");
        assert!(parts.iter().any(|p| p.part == "in-"));
        assert!(parts.iter().any(|p| p.part == "evit"));
        assert!(parts.iter().any(|p| p.part == "-able"));

        let surg_parts = decompose_roots("surgery");
        assert!(surg_parts.iter().any(|p| p.part == "surg"));
        assert!(surg_parts.iter().any(|p| p.part == "-ery"));
    }

    #[test]
    fn levenshtein_computes_expected_distances() {
        assert_eq!(levenshtein("surgery", "surgery"), 0);
        assert_eq!(levenshtein("surgery", "sugary"), 2);
        assert_eq!(levenshtein("surgery", "surely"), 2);
        assert_eq!(levenshtein("surgery", "surety"), 2);
        assert_eq!(levenshtein("surgery", "surge"), 2);
        assert_eq!(levenshtein("meticulous", "meticulus"), 1);
    }

    #[test]
    fn parse_word_family_extracts_word_pos_and_gloss() {
        let (w, pos, gloss) = parse_word_family_entry("microsurgery n. 显微外科");
        assert_eq!(w, "microsurgery");
        assert_eq!(pos.as_deref(), Some("n."));
        assert_eq!(gloss.as_deref(), Some("显微外科"));

        let (w2, pos2, gloss2) = parse_word_family_entry("meticulously adv.");
        assert_eq!(w2, "meticulously");
        assert_eq!(pos2.as_deref(), Some("adv."));
        assert_eq!(gloss2, None);
    }
}
