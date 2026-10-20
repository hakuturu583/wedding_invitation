"""フォント登録と和文組版(禁則つき折り返し・縦書き)のヘルパー."""

from __future__ import annotations

from pathlib import Path

from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont

FONT_DIR = Path(__file__).parent / "fonts"
MINCHO = "ShipporiMincho"
SCRIPT = "GreatVibes"


def register_fonts() -> None:
    """TrueType フォントを登録する(PDF にはサブセット埋め込みされる)."""
    registered = pdfmetrics.getRegisteredFontNames()
    if MINCHO not in registered:
        pdfmetrics.registerFont(TTFont(MINCHO, str(FONT_DIR / "ShipporiMincho-Regular.ttf")))
    if SCRIPT not in registered:
        pdfmetrics.registerFont(TTFont(SCRIPT, str(FONT_DIR / "GreatVibes-Regular.ttf")))


def text_width(text: str, font: str, size: float) -> float:
    return pdfmetrics.stringWidth(text, font, size)


# ---------------------------------------------------------------- 横組み

# 行頭に置かない文字(行頭禁則)。前の行末にぶら下げる。
NO_LINE_START = set("、。，．・：；？！ー－）」』】〕｝〉》ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ々ゝゞ")
# 行末に置かない文字(行末禁則)
NO_LINE_END = set("（「『【〔｛〈《")


def wrap(text: str, font: str, size: float, max_width: float) -> list[str]:
    """改行を保ちつつ max_width に収まるよう 1 文字単位で折り返す."""
    lines: list[str] = []
    for para in text.split("\n"):
        if not para:
            lines.append("")
            continue
        line = ""
        for ch in para:
            if line and text_width(line + ch, font, size) > max_width:
                if ch in NO_LINE_START:
                    line += ch  # ぶら下げ
                    continue
                if line[-1] in NO_LINE_END:
                    lines.append(line[:-1])
                    line = line[-1] + ch
                    continue
                lines.append(line)
                line = ch
            else:
                line += ch
        lines.append(line)
    return lines


# ---------------------------------------------------------------- 縦組み

# 縦書きで 90 度回転させる文字
ROTATE = set("ー－‐-−—―～〜…‥（）()「」『』【】［］[]｛｝{}〈〉《》＝=＜＞<>")
# 小書き仮名(右上に寄せる)
SMALL_KANA = set("ぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶ")
# 句読点(右上に配置)
PUNCT = set("、。，．")

_DIGITS = str.maketrans(
    "0123456789０１２３４５６７８９",
    "〇一二三四五六七八九〇一二三四五六七八九",
)
_HYPHENS = str.maketrans({c: "－" for c in "-‐−—―ｰ"})


def to_vertical(text: str) -> str:
    """縦書き用に正規化する: 数字は漢数字、ハイフンは全角、英字は全角."""
    text = text.translate(_DIGITS).translate(_HYPHENS)
    out = []
    for ch in text:
        if "!" <= ch <= "~":
            ch = chr(ord(ch) + 0xFEE0)
        out.append(ch)
    return "".join(out)


def _advance(ch: str) -> float:
    """1 文字が縦方向に占める長さ(em 単位)."""
    return 0.5 if ch == " " else 1.0


def vertical_length(text: str, tracking: float = 0.0) -> float:
    """縦組みしたときの長さ(em 単位)."""
    return sum(_advance(ch) + tracking for ch in text) - (tracking if text else 0)


def fit_size(text: str, size: float, max_length: float, tracking: float = 0.0,
             min_size: float = 6.0) -> float:
    """縦の長さが max_length(pt) に収まるフォントサイズを返す."""
    units = vertical_length(text, tracking)
    if units <= 0:
        return size
    return max(min_size, min(size, max_length / units))


def draw_vertical(c, text: str, x: float, top: float, font: str, size: float,
                  tracking: float = 0.0) -> float:
    """x を中心線として top から下へ縦書きする。描画後の下端 y を返す."""
    y = top
    for ch in text:
        adv = _advance(ch) * size
        if ch.strip():
            cy = y - size / 2  # 文字ボックス中央
            w = text_width(ch, font, size)
            if ch in ROTATE:
                c.saveState()
                c.translate(x, cy)
                c.rotate(-90)
                c.setFont(font, size)
                c.drawString(-w / 2, -size * 0.36, ch)
                c.restoreState()
            elif ch in PUNCT:
                c.setFont(font, size)
                c.drawString(x - w / 2 + size * 0.6, cy + size * 0.24, ch)
            elif ch in SMALL_KANA:
                c.setFont(font, size)
                c.drawString(x - w / 2 + size * 0.1, cy - size * 0.28, ch)
            else:
                c.setFont(font, size)
                c.drawString(x - w / 2, cy - size * 0.36, ch)
        y -= adv + tracking * size
    return y + tracking * size
