"""招待状のデザイン定義.

色はすべて CMYK (0〜1) で定義する。透明効果は使わない(PDF/X-1a 相当の入稿に対応するため)。
細い線や小さい文字はできるだけ版数を少なくし、総インキ量は 300% 以下に抑える。
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Callable

from reportlab.lib.colors import CMYKColor
from reportlab.lib.units import mm


def cmyk(c: float, m: float, y: float, k: float) -> CMYKColor:
    return CMYKColor(c, m, y, k)


PAPER = cmyk(0, 0, 0, 0)


@dataclass(frozen=True)
class Design:
    key: str
    name: str
    description: str
    background: CMYKColor | None  # None は紙白(インキなし)
    accent: CMYKColor
    sub: CMYKColor
    text: CMYKColor
    title: str
    draw_card: Callable
    draw_envelope: Callable
    title_on_band: bool = False
    bottom_margin: float = 13 * mm  # 本文の下端(装飾と重ならない位置)

    def swatches(self) -> list[str]:
        """UI 表示用の近似 RGB(画面プレビューのみに使用)."""
        cols = [self.background or PAPER, self.accent, self.sub]
        return [_approx_hex(col) for col in cols]


def _approx_hex(col: CMYKColor) -> str:
    c, m, y, k = col.cyan, col.magenta, col.yellow, col.black
    r = 255 * (1 - c) * (1 - k)
    g = 255 * (1 - m) * (1 - k)
    b = 255 * (1 - y) * (1 - k)
    return "#%02x%02x%02x" % (round(r), round(g), round(b))


# ------------------------------------------------------------------ 共通パーツ

def fill_background(c, design: Design, w: float, h: float, bleed: float) -> None:
    if design.background is None:
        return
    c.setFillColor(design.background)
    c.rect(-bleed, -bleed, w + 2 * bleed, h + 2 * bleed, stroke=0, fill=1)


def divider(c, cx: float, y: float, width: float, color: CMYKColor) -> None:
    """中央に菱形を置いた飾り罫."""
    c.setStrokeColor(color)
    c.setFillColor(color)
    c.setLineWidth(0.4)
    gap = 2.2 * mm
    c.line(cx - width / 2, y, cx - gap, y)
    c.line(cx + gap, y, cx + width / 2, y)
    d = 1.1 * mm
    p = c.beginPath()
    p.moveTo(cx, y + d)
    p.lineTo(cx + d, y)
    p.lineTo(cx, y - d)
    p.lineTo(cx - d, y)
    p.close()
    c.drawPath(p, stroke=0, fill=1)


def _corner_flourish(c, x: float, y: float, sx: int, sy: int, s: float) -> None:
    """額縁の角飾り. (x, y) が角、sx/sy は内側への向き(±1)."""
    c.saveState()
    c.translate(x, y)
    c.scale(sx, sy)
    p = c.beginPath()
    p.moveTo(0, 7 * s)
    p.curveTo(0, 2 * s, 2 * s, 0, 7 * s, 0)
    c.drawPath(p, stroke=1, fill=0)
    # 渦巻き
    p = c.beginPath()
    p.moveTo(2.2 * s, 2.2 * s)
    p.curveTo(5 * s, 5 * s, 9 * s, 3.5 * s, 8.5 * s, 1.6 * s)
    p.curveTo(8.2 * s, 0.6 * s, 6.6 * s, 0.8 * s, 6.9 * s, 2.0 * s)
    c.drawPath(p, stroke=1, fill=0)
    p = c.beginPath()
    p.moveTo(2.2 * s, 2.2 * s)
    p.curveTo(5 * s, 5 * s, 3.5 * s, 9 * s, 1.6 * s, 8.5 * s)
    p.curveTo(0.6 * s, 8.2 * s, 0.8 * s, 6.6 * s, 2.0 * s, 6.9 * s)
    c.drawPath(p, stroke=1, fill=0)
    c.circle(2.2 * s, 2.2 * s, 0.55 * s, stroke=0, fill=1)
    c.restoreState()


def _leaf(c, length: float, width: float) -> None:
    """原点から +x 方向に伸びる葉."""
    p = c.beginPath()
    p.moveTo(0, 0)
    p.curveTo(length * 0.3, width, length * 0.75, width * 0.9, length, 0)
    p.curveTo(length * 0.75, -width * 0.9, length * 0.3, -width, 0, 0)
    p.close()
    c.drawPath(p, stroke=0, fill=1)


def _sprig(c, x, y, angle, length, leaf_color, stem_color, n=7, leaf_len=None) -> None:
    """茎に葉が交互につく枝."""
    leaf_len = leaf_len or length * 0.32
    c.saveState()
    c.translate(x, y)
    c.rotate(angle)
    c.setStrokeColor(stem_color)
    c.setLineWidth(0.6)
    p = c.beginPath()
    p.moveTo(0, 0)
    p.curveTo(length * 0.35, length * 0.06, length * 0.7, length * 0.05, length, 0)
    c.drawPath(p, stroke=1, fill=0)
    c.setFillColor(leaf_color)
    for i in range(n):
        t = (i + 1) / (n + 1)
        side = 1 if i % 2 == 0 else -1
        c.saveState()
        c.translate(length * t, length * 0.05 * math.sin(math.pi * t))
        c.rotate(side * 48)
        _leaf(c, leaf_len * (1.05 - 0.4 * t), leaf_len * 0.28)
        c.restoreState()
    c.saveState()
    c.translate(length, 0)
    _leaf(c, leaf_len * 0.7, leaf_len * 0.22)
    c.restoreState()
    c.restoreState()


def _sakura(c, x, y, r, petal, center, rotation=0.0) -> None:
    """桜の花(切れ込みのある 5 枚の花びら)."""
    c.saveState()
    c.translate(x, y)
    c.rotate(rotation)
    c.setFillColor(petal)
    for i in range(5):
        c.saveState()
        c.rotate(i * 72)
        p = c.beginPath()
        p.moveTo(0, 0)
        p.curveTo(-r * 0.55, r * 0.35, -r * 0.6, r * 0.85, -r * 0.22, r)
        p.lineTo(0, r * 0.84)
        p.lineTo(r * 0.22, r)
        p.curveTo(r * 0.6, r * 0.85, r * 0.55, r * 0.35, 0, 0)
        p.close()
        c.drawPath(p, stroke=0, fill=1)
        c.restoreState()
    c.setFillColor(center)
    c.setStrokeColor(center)
    c.setLineWidth(max(0.25, r * 0.03))
    for i in range(5):
        a = math.radians(i * 72 + 36 + 90)
        c.line(0, 0, r * 0.38 * math.cos(a), r * 0.38 * math.sin(a))
        c.circle(r * 0.42 * math.cos(a), r * 0.42 * math.sin(a), r * 0.05, stroke=0, fill=1)
    c.circle(0, 0, r * 0.1, stroke=0, fill=1)
    c.restoreState()


def _petal(c, x, y, r, color, rotation) -> None:
    c.saveState()
    c.translate(x, y)
    c.rotate(rotation)
    c.setFillColor(color)
    p = c.beginPath()
    p.moveTo(0, 0)
    p.curveTo(-r * 0.6, r * 0.4, -r * 0.5, r, -r * 0.18, r * 1.1)
    p.lineTo(0, r * 0.95)
    p.lineTo(r * 0.18, r * 1.1)
    p.curveTo(r * 0.5, r, r * 0.6, r * 0.4, 0, 0)
    p.close()
    c.drawPath(p, stroke=0, fill=1)
    c.restoreState()


# ------------------------------------------------------------------ クラシック

CLASSIC_GOLD = cmyk(0.10, 0.30, 0.75, 0.10)


def _classic_frame(c, w, h, inset, color, s):
    c.setStrokeColor(color)
    c.setFillColor(color)
    c.setLineWidth(0.9)
    c.rect(inset, inset, w - 2 * inset, h - 2 * inset, stroke=1, fill=0)
    c.setLineWidth(0.3)
    i2 = inset + 1.3 * mm
    c.rect(i2, i2, w - 2 * i2, h - 2 * i2, stroke=1, fill=0)
    c.setLineWidth(0.45)
    i3 = i2 + 1.0 * mm
    for x, y, sx, sy in ((i3, i3, 1, 1), (w - i3, i3, -1, 1), (i3, h - i3, 1, -1), (w - i3, h - i3, -1, -1)):
        _corner_flourish(c, x, y, sx, sy, s)


def classic_card(c, d: Design, w, h, bleed):
    fill_background(c, d, w, h, bleed)
    _classic_frame(c, w, h, 5 * mm, d.accent, 0.9 * mm)


def classic_envelope(c, d: Design, w, h):
    c.setStrokeColor(d.accent)
    c.setLineWidth(0.5)
    c.rect(7 * mm, 7 * mm, w - 14 * mm, h - 14 * mm, stroke=1, fill=0)
    c.setLineWidth(0.4)
    c.setFillColor(d.accent)
    for x, y, sx, sy in ((8 * mm, 8 * mm, 1, 1), (w - 8 * mm, h - 8 * mm, -1, -1)):
        _corner_flourish(c, x, y, sx, sy, 0.8 * mm)


# ------------------------------------------------------------------ ボタニカル

LEAF = cmyk(0.55, 0.15, 0.60, 0.15)
LEAF_LIGHT = cmyk(0.35, 0.05, 0.40, 0.05)
STEM = cmyk(0.50, 0.20, 0.55, 0.35)
BERRY = cmyk(0.05, 0.35, 0.25, 0.05)


def botanical_card(c, d: Design, w, h, bleed):
    fill_background(c, d, w, h, bleed)
    # 左上と右下の角に、辺に沿わせて枝を配置(本文にかからないように)
    _sprig(c, -3 * mm, h - 4 * mm, -8, 34 * mm, LEAF, STEM, n=6, leaf_len=8 * mm)
    _sprig(c, 3 * mm, h + 3 * mm, -80, 40 * mm, LEAF_LIGHT, STEM, n=6, leaf_len=8 * mm)
    _sprig(c, w + 3 * mm, 4 * mm, 172, 34 * mm, LEAF, STEM, n=6, leaf_len=8 * mm)
    _sprig(c, w - 3 * mm, -3 * mm, 100, 40 * mm, LEAF_LIGHT, STEM, n=6, leaf_len=8 * mm)
    c.setFillColor(BERRY)
    for x, y, r in ((9 * mm, h - 9 * mm, 1.3), (12.5 * mm, h - 7 * mm, 1.0),
                    (w - 9 * mm, 9 * mm, 1.3), (w - 12.5 * mm, 7 * mm, 1.0)):
        c.circle(x, y, r * mm, stroke=0, fill=1)
    c.setStrokeColor(d.sub)
    c.setLineWidth(0.35)
    c.rect(6 * mm, 6 * mm, w - 12 * mm, h - 12 * mm, stroke=1, fill=0)


def botanical_envelope(c, d: Design, w, h):
    _sprig(c, 6 * mm, 8 * mm, 35, 30 * mm, LEAF, STEM, n=6)
    _sprig(c, 8 * mm, 7 * mm, 12, 22 * mm, LEAF_LIGHT, STEM, n=5)
    c.setFillColor(BERRY)
    c.circle(10 * mm, 15 * mm, 1.1 * mm, stroke=0, fill=1)


# ------------------------------------------------------------------ 桜

SAKURA = cmyk(0.0, 0.32, 0.10, 0.0)
SAKURA_LIGHT = cmyk(0.0, 0.16, 0.05, 0.0)
SAKURA_CENTER = cmyk(0.0, 0.70, 0.30, 0.10)


def sakura_card(c, d: Design, w, h, bleed):
    fill_background(c, d, w, h, bleed)
    flowers = [
        (w - 6 * mm, h - 7 * mm, 9 * mm, SAKURA, 10), (w - 20 * mm, h - 4 * mm, 6 * mm, SAKURA_LIGHT, 40),
        (w - 4 * mm, h - 21 * mm, 5.5 * mm, SAKURA_LIGHT, 25), (w - 16 * mm, h - 14 * mm, 3.2 * mm, SAKURA, 60),
        (6 * mm, 6 * mm, 8 * mm, SAKURA, 30), (19 * mm, 3 * mm, 5 * mm, SAKURA_LIGHT, 5),
        (3 * mm, 19 * mm, 4.5 * mm, SAKURA_LIGHT, 50),
    ]
    for x, y, r, col, rot in flowers:
        _sakura(c, x, y, r, col, SAKURA_CENTER, rot)
    for x, y, r, rot in ((w - 30 * mm, h - 13 * mm, 1.6 * mm, 30), (w - 9 * mm, h - 31 * mm, 1.4 * mm, 120),
                         (30 * mm, 7 * mm, 1.4 * mm, 200), (8 * mm, 28 * mm, 1.2 * mm, 280),
                         (w - 26 * mm, h - 25 * mm, 1.1 * mm, 75)):
        _petal(c, x, y, r, SAKURA, rot)
    c.setStrokeColor(d.accent)
    c.setLineWidth(0.35)
    c.rect(5 * mm, 5 * mm, w - 10 * mm, h - 10 * mm, stroke=1, fill=0)


def sakura_envelope(c, d: Design, w, h):
    _sakura(c, 12 * mm, 13 * mm, 6 * mm, SAKURA, SAKURA_CENTER, 20)
    _sakura(c, 21 * mm, 9 * mm, 3.8 * mm, SAKURA_LIGHT, SAKURA_CENTER, 50)
    _petal(c, 24 * mm, 17 * mm, 1.4 * mm, SAKURA, 210)


# ------------------------------------------------------------------ モダン

NAVY = cmyk(1.0, 0.80, 0.30, 0.30)
MODERN_GOLD = cmyk(0.15, 0.30, 0.65, 0.05)
MODERN_BAND = 24 * mm


def modern_card(c, d: Design, w, h, bleed):
    c.setFillColor(NAVY)
    c.rect(-bleed, h - MODERN_BAND, w + 2 * bleed, MODERN_BAND + bleed, stroke=0, fill=1)
    c.rect(-bleed, -bleed, w + 2 * bleed, 6 * mm + bleed, stroke=0, fill=1)
    c.setStrokeColor(MODERN_GOLD)
    c.setLineWidth(0.6)
    c.line(-bleed, h - MODERN_BAND - 1.2 * mm, w + bleed, h - MODERN_BAND - 1.2 * mm)
    c.line(-bleed, 6 * mm + 1.2 * mm, w + bleed, 6 * mm + 1.2 * mm)


def modern_envelope(c, d: Design, w, h):
    c.setFillColor(NAVY)
    c.rect(8 * mm, 8 * mm, 1.2 * mm, 26 * mm, stroke=0, fill=1)
    c.setFillColor(MODERN_GOLD)
    c.rect(10.2 * mm, 8 * mm, 0.4 * mm, 26 * mm, stroke=0, fill=1)


# ------------------------------------------------------------------ 一覧

TEXT_BLACK = cmyk(0, 0, 0, 0.88)

DESIGNS: dict[str, Design] = {
    d.key: d
    for d in (
        Design("classic", "クラシック", "アイボリーにゴールドの額縁飾り。格式ある王道デザイン",
               cmyk(0.0, 0.03, 0.10, 0.0), CLASSIC_GOLD, CLASSIC_GOLD,
               cmyk(0.0, 0.10, 0.20, 0.82), "Wedding Invitation", classic_card, classic_envelope),
        Design("botanical", "ボタニカル", "ユーカリの枝葉をあしらったナチュラルなデザイン",
               cmyk(0.0, 0.0, 0.03, 0.0), LEAF, LEAF_LIGHT,
               cmyk(0.20, 0.0, 0.15, 0.80), "Wedding Invitation", botanical_card, botanical_envelope,
               bottom_margin=17 * mm),
        Design("sakura", "桜", "淡い桜色に桜の花が舞う和のデザイン",
               cmyk(0.0, 0.05, 0.02, 0.0), SAKURA_CENTER, SAKURA,
               cmyk(0.0, 0.15, 0.05, 0.82), "Wedding Invitation", sakura_card, sakura_envelope,
               bottom_margin=17 * mm),
        Design("modern", "モダン", "ネイビーの帯にゴールドのラインを効かせたシンプルモダン",
               None, NAVY, MODERN_GOLD,
               cmyk(0.30, 0.20, 0.0, 0.80), "Wedding Invitation", modern_card, modern_envelope,
               title_on_band=True, bottom_margin=15 * mm),
    )
}

DEFAULT_DESIGN = "classic"


def get_design(key: str | None) -> Design:
    return DESIGNS.get(key or DEFAULT_DESIGN, DESIGNS[DEFAULT_DESIGN])
