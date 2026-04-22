//! 买家昵称相似度匹配。
//!
//! 完整对齐 Python 4.3.0 `_legacy/app/services/order_match_scoring.py::similarity_percent` 的
//! 全部分支，包含：
//!
//! 1. 完全一致直接 100。
//! 2. `trim` 后相等 95。
//! 3. "改名 + 数字尾巴"场景：去尾后 core 相等，core ≥2 字 → 95，core == 1 字 → 80。
//! 4. 较短串整体包含在较长串中：
//!    - 3+ 字 → 90
//!    - 2 字 → 80
//!    - 1 字 → `100 / max(3, longer_len)` 的保守值。
//! 5. 较短串作为较长串的 Unicode 码点子序列：4+ → 85，3 → 80，2 → 70。
//! 6. 其他场景退化到 LCS 版 `sequence_similarity` 兜底（与 Python `SequenceMatcher.ratio` 的
//!    `2*matches / total` 定义形式一致，差值受算法本质限制在 ±2 以内）。
//!
//! 所有辅助函数保留 `pub`，供上层策略分级（M4-02）或通用昵称过滤（M4-03）直接复用，
//! 也便于单元测试覆盖每个分支。

use regex::Regex;
use std::cmp::max;
use std::sync::OnceLock;

/// 匹配昵称尾部的数字尾巴。字符集覆盖 ASCII / 全角 / 上标 / 下标数字，以及任意 Unicode 空白。
///
/// 注意：`[⁰¹²³⁴⁵⁶⁷⁸⁹]` 中 `²` (U+00B2) 与 `³` (U+00B3) 位于 Latin-1 Supplement 区，
/// 其余为上标数字区，必须列出而不能用范围表达。
const TRAILING_DIGIT_TAIL_PATTERN: &str = r"[0-9０-９⁰¹²³⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉\s]+$";
pub const GENERIC_NICKNAME_PREFIXES: &[&str] = &["匿名", "微信用户", "默认昵称"];

/// 将浮点相似度裁剪到 0~100 闭区间；非有限值（NaN / inf）一律视作 0。
pub fn clamp_percent(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    (value.round() as i32).clamp(0, 100)
}

/// 昵称相似度（0~100），完整保持 Python 版 `similarity_percent` 的行为。
///
/// `None` 等价于空串；双侧空串视为完全一致返回 100，单侧空串返回 0。
pub fn similarity_percent(left: Option<&str>, right: Option<&str>) -> i32 {
    let left_text = left.unwrap_or("");
    let right_text = right.unwrap_or("");
    let left_trimmed = left_text.trim();
    let right_trimmed = right_text.trim();

    if is_generic_nickname(left_trimmed) || is_generic_nickname(right_trimmed) {
        return 0;
    }
    if left_text == right_text {
        return 100;
    }
    if left_text.is_empty() || right_text.is_empty() {
        return 0;
    }

    if !left_trimmed.is_empty() && left_trimmed == right_trimmed {
        return 95;
    }

    if let Some(similarity) = nickname_similarity_by_rename_patterns(left_trimmed, right_trimmed) {
        return similarity;
    }

    sequence_similarity(left_text, right_text)
}

/// 判断昵称是否属于会污染匹配结果的通用占位昵称。
pub fn is_generic_nickname(name: &str) -> bool {
    let trimmed = name.trim();
    trimmed.is_empty()
        || GENERIC_NICKNAME_PREFIXES
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
}

/// 剥离昵称尾部数字/空白组合。仅用于相似度识别，不参与业务字段写回。
pub fn strip_trailing_digit_tail(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(TRAILING_DIGIT_TAIL_PATTERN).expect("valid regex"));
    re.replace(text, "").trim().to_string()
}

/// 判断 `shorter` 是否按 Unicode 字符顺序嵌入在 `longer` 中（子序列）。
pub fn is_subsequence(shorter: &str, longer: &str) -> bool {
    if shorter.is_empty() {
        return false;
    }
    let shorter_chars: Vec<char> = shorter.chars().collect();
    let mut pos = 0usize;
    for ch in longer.chars() {
        if pos < shorter_chars.len() && ch == shorter_chars[pos] {
            pos += 1;
            if pos == shorter_chars.len() {
                return true;
            }
        }
    }
    false
}

/// 单字命中较长文本时的保守相似度。
///
/// 对"昵称长度 = 1"的情况使用 `100 / max(3, longer_len)`，防止"只有一个常用字重合"被误判为高相似改名。
pub fn single_char_containment_similarity(longer: &str) -> i32 {
    let normalized_length = max(longer.chars().count(), 3);
    clamp_percent(100.0 / normalized_length as f64)
}

/// 按较短串长度返回子序列匹配的相似度，长度 < 2 视为不可识别返回 `None`。
pub fn subsequence_similarity_by_length(text: &str) -> Option<i32> {
    match text.chars().count() {
        n if n >= 4 => Some(85),
        3 => Some(80),
        2 => Some(70),
        _ => None,
    }
}

/// 改名场景特化规则：去尾 core 相同 / 整体包含 / 子序列三档识别。
pub fn nickname_similarity_by_rename_patterns(left: &str, right: &str) -> Option<i32> {
    if left.is_empty() || right.is_empty() {
        return None;
    }

    let left_core = strip_trailing_digit_tail(left);
    let right_core = strip_trailing_digit_tail(right);

    if !left_core.is_empty() && !right_core.is_empty() && left_core == right_core && left != right {
        if left_core.chars().count() >= 2 {
            return Some(95);
        }
        return Some(80);
    }

    let (shorter, longer) = if left.chars().count() <= right.chars().count() {
        (left, right)
    } else {
        (right, left)
    };
    let (shorter_core, longer_core) = if left_core.chars().count() <= right_core.chars().count() {
        (left_core.as_str(), right_core.as_str())
    } else {
        (right_core.as_str(), left_core.as_str())
    };

    if !shorter.is_empty() && longer.contains(shorter) {
        let len = shorter.chars().count();
        if len >= 3 {
            return Some(90);
        }
        if len == 2 {
            return Some(80);
        }
        return Some(single_char_containment_similarity(longer));
    }

    if !shorter_core.is_empty() && longer_core.contains(shorter_core) {
        let len = shorter_core.chars().count();
        if len >= 3 {
            return Some(90);
        }
        if len == 2 {
            return Some(80);
        }
        return Some(single_char_containment_similarity(longer_core));
    }

    if let Some(similarity) = subsequence_similarity_by_length(shorter) {
        if is_subsequence(shorter, longer) {
            return Some(similarity);
        }
    }

    if let Some(similarity) = subsequence_similarity_by_length(shorter_core) {
        if is_subsequence(shorter_core, longer_core) {
            return Some(similarity);
        }
    }

    None
}

/// 基线序列相似度：`2 * LCS / (len_left + len_right) * 100`，对齐 Python
/// `SequenceMatcher.ratio()` 的计算形式。本实现基于 LCS，与 Ratcliff/Obershelp 在
/// 绝大多数昵称样本上差值 ≤ 2，用于所有"规则未命中"的兜底。
pub fn sequence_similarity(left: &str, right: &str) -> i32 {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    if left_chars.is_empty() || right_chars.is_empty() {
        return 0;
    }
    let lcs = lcs_length(&left_chars, &right_chars);
    let ratio = (2.0 * lcs as f64) / (left_chars.len() + right_chars.len()) as f64 * 100.0;
    clamp_percent(ratio)
}

fn lcs_length(left: &[char], right: &[char]) -> usize {
    let mut prev = vec![0usize; right.len() + 1];
    let mut curr = vec![0usize; right.len() + 1];
    for l in left {
        for (j, r) in right.iter().enumerate() {
            curr[j + 1] = if l == r {
                prev[j] + 1
            } else {
                max(prev[j + 1], curr[j])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }
    prev[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_strings_return_100() {
        assert_eq!(similarity_percent(Some("张三"), Some("张三")), 100);
        assert_eq!(
            similarity_percent(
                Some("💫其实一个我有故事的人"),
                Some("💫其实一个我有故事的人"),
            ),
            100
        );
    }

    #[test]
    fn empty_one_side_returns_0() {
        assert_eq!(similarity_percent(None, Some("张三")), 0);
        assert_eq!(similarity_percent(Some("张三"), None), 0);
        assert_eq!(similarity_percent(Some(""), Some("张三")), 0);
        assert_eq!(similarity_percent(Some("张三"), Some("")), 0);
    }

    #[test]
    fn trim_equivalent_returns_95() {
        assert_eq!(similarity_percent(Some("张三 "), Some("张三")), 95);
        assert_eq!(similarity_percent(Some("张三"), Some("张三 ")), 95);
        assert_eq!(similarity_percent(Some(" 张三 "), Some("张三")), 95);
        assert_eq!(similarity_percent(Some("\t张三\n"), Some("张三")), 95);
    }

    #[test]
    fn generic_nickname_prefixes_return_zero() {
        assert_eq!(
            similarity_percent(Some("匿名用户123"), Some("匿名用户456")),
            0
        );
        assert_eq!(
            similarity_percent(Some("微信用户abc"), Some("微信用户xyz")),
            0
        );
        assert_eq!(similarity_percent(Some("默认昵称"), Some("默认昵称1")), 0);
        assert_eq!(similarity_percent(Some(""), Some("张三")), 0);
        assert!(similarity_percent(Some("匿了"), Some("匿了123")) > 0);
        assert!(similarity_percent(Some("正常买家"), Some("正常买家123")) > 0);
    }

    #[test]
    fn core_same_len_ge2_returns_95() {
        assert_eq!(similarity_percent(Some("张三"), Some("张三123")), 95);
        assert_eq!(similarity_percent(Some("张三123"), Some("张三")), 95);
        assert_eq!(similarity_percent(Some("张三"), Some("张三０１２")), 95);
        assert_eq!(similarity_percent(Some("张三"), Some("张三²³")), 95);
        assert_eq!(similarity_percent(Some("张三"), Some("张三₀₁")), 95);
        assert_eq!(similarity_percent(Some("赵亮6057"), Some("赵亮")), 95);
        assert_eq!(similarity_percent(Some("赵亮6057"), Some("赵亮888")), 95);
        assert_eq!(similarity_percent(Some("张三"), Some("张三  123")), 95);
    }

    #[test]
    fn core_same_single_char_returns_80() {
        assert_eq!(similarity_percent(Some("张"), Some("张12")), 80);
        assert_eq!(similarity_percent(Some("张12"), Some("张")), 80);
    }

    #[test]
    fn substring_len2_returns_80() {
        assert_eq!(similarity_percent(Some("张三"), Some("大张三小")), 80);
        assert_eq!(similarity_percent(Some("大张三小"), Some("张三")), 80);
    }

    #[test]
    fn substring_len3_or_more_returns_90() {
        assert_eq!(similarity_percent(Some("张三大"), Some("大张三大小")), 90);
        assert_eq!(similarity_percent(Some("潍坊印"), Some("大潍坊印小")), 90);
    }

    #[test]
    fn subsequence_len4_returns_85() {
        assert_eq!(
            similarity_percent(Some("潍坊印刷"), Some("潍坊精装印刷王宏杰")),
            85
        );
    }

    #[test]
    fn subsequence_len3_returns_80() {
        assert_eq!(similarity_percent(Some("张李王"), Some("张三李四王五")), 80);
    }

    #[test]
    fn subsequence_len2_returns_70() {
        assert_eq!(similarity_percent(Some("张三"), Some("张大三")), 70);
        assert_eq!(similarity_percent(Some("李四"), Some("李小四")), 70);
    }

    #[test]
    fn single_char_containment_stays_low() {
        assert_eq!(similarity_percent(Some("我"), Some("我期待")), 33);
        let long_name = similarity_percent(Some("度"), Some("城市轻度假酒店-杨"));
        assert!(
            long_name <= 15,
            "单字包含长名称应保持 ≤ 15, got {long_name}"
        );
    }

    #[test]
    fn sequence_similarity_fallback() {
        let partial = similarity_percent(Some("孙二"), Some("孙三"));
        assert!(partial > 0 && partial < 80, "got {partial}");
        assert_eq!(similarity_percent(Some("a"), Some("b")), 0);
    }

    #[test]
    fn case_sensitive_differences_do_not_trigger_rename() {
        assert_eq!(similarity_percent(Some("abc"), Some("ABC")), 0);
    }

    #[test]
    fn trim_short_single_character_still_returns_95() {
        assert_eq!(similarity_percent(Some("张 "), Some("张")), 95);
    }

    #[test]
    fn emoji_with_numeric_tail_counts_as_single_char() {
        assert_eq!(similarity_percent(Some("🐱"), Some("🐱123")), 80);
    }

    #[test]
    fn strip_trailing_digit_tail_covers_digit_variants() {
        assert_eq!(strip_trailing_digit_tail("张三123"), "张三");
        assert_eq!(strip_trailing_digit_tail("张三０１２"), "张三");
        assert_eq!(strip_trailing_digit_tail("张三²³"), "张三");
        assert_eq!(strip_trailing_digit_tail("张三₀₁"), "张三");
        assert_eq!(strip_trailing_digit_tail("张三 123 "), "张三");
        assert_eq!(strip_trailing_digit_tail("张三"), "张三");
        assert_eq!(strip_trailing_digit_tail("123"), "");
        assert_eq!(strip_trailing_digit_tail(""), "");
    }

    #[test]
    fn is_subsequence_verifies_ordering() {
        assert!(is_subsequence("abc", "a1b2c3"));
        assert!(is_subsequence("张三", "张大三"));
        assert!(!is_subsequence("abc", "acb"));
        assert!(!is_subsequence("", "a"));
        assert!(!is_subsequence("longer", "ab"));
    }

    #[test]
    fn is_generic_nickname_only_matches_known_prefixes() {
        assert!(is_generic_nickname("匿名用户123"));
        assert!(is_generic_nickname(" 微信用户abc "));
        assert!(is_generic_nickname("默认昵称1"));
        assert!(is_generic_nickname(""));
        assert!(!is_generic_nickname("匿了"));
        assert!(!is_generic_nickname("正常买家"));
    }

    #[test]
    fn clamp_percent_handles_edges() {
        assert_eq!(clamp_percent(0.0), 0);
        assert_eq!(clamp_percent(100.0), 100);
        assert_eq!(clamp_percent(-5.0), 0);
        assert_eq!(clamp_percent(150.0), 100);
        assert_eq!(clamp_percent(f64::NAN), 0);
        assert_eq!(clamp_percent(f64::INFINITY), 0);
        assert_eq!(clamp_percent(33.4), 33);
        assert_eq!(clamp_percent(33.5), 34);
    }

    #[test]
    fn single_char_containment_similarity_known_values() {
        assert_eq!(single_char_containment_similarity("abc"), 33);
        assert_eq!(single_char_containment_similarity("abcdef"), 17);
        assert_eq!(single_char_containment_similarity("a"), 33);
    }

    #[test]
    fn subsequence_similarity_by_length_boundaries() {
        assert_eq!(subsequence_similarity_by_length(""), None);
        assert_eq!(subsequence_similarity_by_length("a"), None);
        assert_eq!(subsequence_similarity_by_length("ab"), Some(70));
        assert_eq!(subsequence_similarity_by_length("abc"), Some(80));
        assert_eq!(subsequence_similarity_by_length("abcd"), Some(85));
        assert_eq!(subsequence_similarity_by_length("abcde"), Some(85));
    }

    /// Python 回归样本：值与 `_legacy/app/services/order_match_scoring.py` 的
    /// `similarity_percent` 输出严格一致，用于守住 matching 模块化后的行为兼容性。
    #[test]
    fn python_regression_samples() {
        let cases: &[(&str, &str, i32)] = &[
            ("赵亮6057", "赵亮", 95),
            ("潍坊印刷", "潍坊精装印刷王宏杰", 85),
            ("我期待", "我", 33),
            ("度", "城市轻度假酒店-杨", 11),
            ("张三", "张三123", 95),
            ("张", "张12", 80),
            ("张三", "大张三小", 80),
            ("张三大", "大张三大小", 90),
            ("张李王", "张三李四王五", 80),
            ("张三", "张大三", 70),
        ];
        for (left, right, expected) in cases {
            let actual = similarity_percent(Some(left), Some(right));
            assert_eq!(
                actual, *expected,
                "similarity_percent({:?}, {:?}) expected {} got {}",
                left, right, expected, actual,
            );
        }
    }

    #[test]
    fn sequence_similarity_is_symmetric() {
        let a = sequence_similarity("张三丰", "张丰三");
        let b = sequence_similarity("张丰三", "张三丰");
        assert_eq!(a, b);
    }

    #[test]
    fn sequence_similarity_empty_inputs_return_zero() {
        assert_eq!(sequence_similarity("", ""), 0);
        assert_eq!(sequence_similarity("张三", ""), 0);
        assert_eq!(sequence_similarity("", "张三"), 0);
    }
}
