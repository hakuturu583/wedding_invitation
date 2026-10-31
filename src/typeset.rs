//! 和文組版: 禁則つきの折り返しと縦書き.

use crate::canvas::Canvas;
use crate::fonts::{self, FontId};

// ------------------------------------------------------------------ 横組み

/// 行頭に置かない文字(行頭禁則)。前の行末にぶら下げる.
const NO_LINE_START: &str = "、。，．・：；？！ー－）」』】〕｝〉》ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ々ゝゞ";
/// 行末に置かない文字(行末禁則).
const NO_LINE_END: &str = "（「『【〔｛〈《";

/// 改行を保ちつつ max_width に収まるよう 1 文字単位で折り返す.
pub fn wrap(text: &str, font: FontId, size: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut width = 0.0;
        for ch in para.chars() {
            let w = fonts::text_width(font, ch.encode_utf8(&mut [0; 4]), size);
            if !line.is_empty() && width + w > max_width {
                if NO_LINE_START.contains(ch) {
                    line.push(ch); // ぶら下げ
                    width += w;
                    continue;
                }
                let last = line.chars().last().unwrap();
                if NO_LINE_END.contains(last) && line.chars().count() > 1 {
                    line.pop();
                    lines.push(std::mem::take(&mut line));
                    line.push(last);
                } else {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(ch);
                width = fonts::text_width(font, &line, size);
            } else {
                line.push(ch);
                width += w;
            }
        }
        lines.push(line);
    }
    lines
}

/// 文節を意識した折り返し(写真の説明など短い文向け).
///
/// 行数はできるだけ少なくしたうえで、空白・句読点・助詞のあとで改行し、
/// 単語の途中(「お越|し」「エ|スカレーター」など)では切らないようにする。
/// どうしても収まらない段落は 1 文字単位の wrap に任せる。
pub fn wrap_phrases(text: &str, font: FontId, size: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        match phrase_lines(para, font, size, max_width) {
            Some(mut l) => lines.append(&mut l),
            None => lines.extend(wrap(para, font, size, max_width)),
        }
    }
    lines
}

#[derive(Clone, Copy, PartialEq)]
enum Class {
    Hira,
    Kata,
    Kanji,
    Alnum,
    Space,
    Other,
}

fn class(ch: char) -> Class {
    match ch {
        ' ' | '　' => Class::Space,
        'ぁ'..='ゖ' => Class::Hira,
        'ァ'..='ヺ' | 'ー' => Class::Kata,
        '々' | '\u{4E00}'..='\u{9FFF}' => Class::Kanji,
        c if c.is_ascii_alphanumeric() || ":：.／/-+".contains(c) => Class::Alnum,
        'Ａ'..='Ｚ' | 'ａ'..='ｚ' | '０'..='９' => Class::Alnum,
        _ => Class::Other,
    }
}

/// chars[i - 1] と chars[i] の間で改行するときの減点(None は禁則で改行できない).
fn break_penalty(chars: &[char], i: usize) -> Option<f32> {
    let (a, b) = (chars[i - 1], chars[i]);
    if NO_LINE_START.contains(b) || NO_LINE_END.contains(a) {
        return None;
    }
    let (ca, cb) = (class(a), class(b));
    if ca == Class::Space {
        // 「6F ロビー」「3F 北改札口」のような階数・番号は後ろの語と離さない
        let label = i >= 2 && class(chars[i - 2]) == Class::Alnum && class(b) != Class::Alnum;
        return Some(if label { 8.0 } else { 0.0 });
    }
    if "、。，．！？）」』".contains(a) {
        return Some(0.0);
    }
    if cb == Class::Space {
        return Some(0.5);
    }
    if ca == Class::Hira && cb != Class::Hira {
        // 「お越し」「ご案内」の「お」「ご」は後ろの語と離さない
        let prefix = "おご".contains(a)
            && (i < 2 || class(chars[i - 2]) != Class::Hira || "にをがはへもでとの".contains(chars[i - 2]));
        if prefix {
            return Some(25.0);
        }
        let particle = "にをがはへもでとのらりて".contains(a);
        return Some(if particle { 1.5 } else { 4.0 }); // 「〜に|御堂筋」「〜の|チャペル」
    }
    if ca == Class::Hira {
        // 助詞のあと(前が漢字・カタカナ、または「から」「まで」など)
        let before: String = chars[i.saturating_sub(3)..i].iter().collect();
        let particle = "にをがはへもでとの".contains(a);
        let after_word = i >= 2 && class(chars[i - 2]) != Class::Hira;
        let compound = ["から", "まで", "より", "ので", "には", "では", "までに"]
            .iter()
            .any(|p| before.ends_with(p));
        if (particle && after_word) || compound {
            return Some(2.0);
        }
        return Some(25.0);
    }
    if ca != cb && ca != Class::Other && cb != Class::Other {
        return Some(if cb == Class::Hira { 25.0 } else { 6.0 });
    }
    Some(25.0)
}

fn phrase_lines(para: &str, font: FontId, size: f32, max_width: f32) -> Option<Vec<String>> {
    let chars: Vec<char> = para.chars().collect();
    let n = chars.len();
    if n == 0 {
        return Some(vec![String::new()]);
    }
    let line_text = |j: usize, i: usize| -> String { chars[j..i].iter().collect::<String>().trim().to_string() };
    // best[i] = chars[..i] を組んだときの (費用, 直前の改行位置)
    let mut best: Vec<Option<(f32, usize)>> = vec![None; n + 1];
    best[0] = Some((0.0, 0));
    for i in 1..=n {
        let penalty = if i == n {
            0.0
        } else {
            match break_penalty(&chars, i) {
                Some(p) => p,
                None => continue,
            }
        };
        for j in (0..i).rev() {
            let Some((before, _)) = best[j] else { continue };
            let text = line_text(j, i);
            let width = fonts::text_width(font, &text, size);
            if width > max_width {
                break;
            }
            // 1 行増えるごとに大きく減点し、行数を最小にする。最終行以外は余白も少し減点
            let slack = if i == n { 0.0 } else { (max_width - width) / max_width };
            let cost = before + 1000.0 + penalty + slack * slack * 4.0;
            if best[i].is_none_or(|(c, _)| cost < c) {
                best[i] = Some((cost, j));
            }
        }
    }
    best[n]?;
    let mut lines = Vec::new();
    let mut i = n;
    while i > 0 {
        let (_, j) = best[i]?;
        lines.push(line_text(j, i));
        i = j;
    }
    lines.reverse();
    Some(lines)
}

// ------------------------------------------------------------------ 縦組み

/// 縦書きで 90 度回転させる文字.
const ROTATE: &str = "ー－‐-−—―～〜…‥（）()「」『』【】［］[]｛｝{}〈〉《》＝=＜＞<>";
/// 小書き仮名(右上に寄せる).
const SMALL_KANA: &str = "ぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶ";
/// 句読点(右上に配置).
const PUNCT: &str = "、。，．";

/// 縦書き用に正規化する: 数字は漢数字、ハイフンは全角、英字は全角.
pub fn to_vertical(text: &str) -> String {
    const KANJI: [char; 10] = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    text.chars()
        .map(|ch| match ch {
            '0'..='9' => KANJI[ch as usize - '0' as usize],
            '０'..='９' => KANJI[ch as usize - '０' as usize],
            '-' | '‐' | '−' | '—' | '―' | 'ｰ' => '－',
            '!'..='~' => char::from_u32(ch as u32 + 0xFEE0).unwrap_or(ch),
            _ => ch,
        })
        .collect()
}

/// 1 文字が縦方向に占める長さ(em 単位).
fn advance(ch: char) -> f32 {
    if ch == ' ' {
        0.5
    } else {
        1.0
    }
}

/// 縦組みしたときの長さ(em 単位).
pub fn vertical_length(text: &str, tracking: f32) -> f32 {
    let n = text.chars().count();
    if n == 0 {
        return 0.0;
    }
    text.chars().map(|c| advance(c) + tracking).sum::<f32>() - tracking
}

/// 縦の長さが max_length(pt) に収まるフォントサイズを返す.
pub fn fit_size(text: &str, size: f32, max_length: f32, tracking: f32) -> f32 {
    let units = vertical_length(text, tracking);
    if units <= 0.0 {
        return size;
    }
    size.min(max_length / units).max(6.0)
}

/// x を中心線として top から下へ縦書きする。描画後の下端 y を返す.
pub fn draw_vertical(c: &mut Canvas, text: &str, x: f32, top: f32, font: FontId, size: f32, tracking: f32) -> f32 {
    let mut y = top;
    c.set_font(font, size);
    for ch in text.chars() {
        let adv = advance(ch) * size;
        if !ch.is_whitespace() {
            let cy = y - size / 2.0; // 文字ボックスの中央
            let s = ch.to_string();
            let w = fonts::text_width(font, &s, size);
            if ROTATE.contains(ch) {
                c.save();
                c.translate(x, cy);
                c.rotate(-90.0);
                c.draw_string(-w / 2.0, -size * 0.36, &s);
                c.restore();
            } else if PUNCT.contains(ch) {
                c.draw_string(x - w / 2.0 + size * 0.6, cy + size * 0.24, &s);
            } else if SMALL_KANA.contains(ch) {
                c.draw_string(x - w / 2.0 + size * 0.1, cy - size * 0.28, &s);
            } else {
                c.draw_string(x - w / 2.0, cy - size * 0.36, &s);
            }
        }
        y -= adv + tracking * size;
    }
    y + tracking * size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_normalization() {
        assert_eq!(to_vertical("1-23-45 A棟"), "一－二三－四五 Ａ棟");
    }

    #[test]
    fn kinsoku() {
        fonts::init_from_repo();
        let lines = wrap("あいうえお。かきくけこ", FontId::Mincho, 10.0, 50.0);
        assert!(lines.iter().all(|l| !l.starts_with('。')), "{lines:?}");
        let lines = wrap("あいう「えお", FontId::Mincho, 10.0, 40.0);
        assert!(lines.iter().all(|l| !l.ends_with('「')), "{lines:?}");
        assert_eq!(wrap("a\n\nb", FontId::Mincho, 10.0, 100.0), vec!["a", "", "b"]);
    }

    #[test]
    fn phrase_wrap_avoids_breaking_words() {
        fonts::init_from_repo();
        let size = 6.4;
        let w = fonts::text_width(FontId::Mincho, "同じ階にあるスイスホテル直結", size);
        let lines = wrap_phrases(
            "同じ階にあるスイスホテル直結エスカレーターから 6F ロビーに向かってください",
            FontId::Mincho,
            size,
            w,
        );
        assert!(lines.iter().all(|l| !l.starts_with("スカレーター")), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.ends_with("から") || l.ends_with("に")),
            "{lines:?}"
        );
        let w = fonts::text_width(FontId::Mincho, "南海なんば駅 3F 北改札口にお越", size);
        let lines = wrap_phrases("南海なんば駅 3F 北改札口にお越しください", FontId::Mincho, size, w);
        assert!(
            lines.iter().all(|l| !l.ends_with("お越") && !l.ends_with("3F")),
            "{lines:?}"
        );
        // 入らない長い単語は 1 文字単位に戻る
        let lines = wrap_phrases("あいうえおかきくけこ", FontId::Mincho, 10.0, 30.0);
        assert!(lines.len() >= 3, "{lines:?}");
    }

    #[test]
    fn vertical_length_counts_half_spaces() {
        assert_eq!(vertical_length("山田 太郎", 0.0), 4.5);
        assert_eq!(vertical_length("", 0.1), 0.0);
    }
}
