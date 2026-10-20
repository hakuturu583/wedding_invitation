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
    fn vertical_length_counts_half_spaces() {
        assert_eq!(vertical_length("山田 太郎", 0.0), 4.5);
        assert_eq!(vertical_length("", 0.1), 0.0);
    }
}
