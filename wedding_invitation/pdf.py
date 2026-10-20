"""印刷入稿用 PDF の生成.

- 色空間はすべて DeviceCMYK (ReportLab の enforceColorSpace で RGB/グレーの混入を禁止)
- フォントはサブセット埋め込み
- 招待状カードは塗り足し 3mm + 日本式トンボ、TrimBox/BleedBox を設定
- 封筒は既製封筒への印刷を想定し仕上がりサイズ、宛名は縦書き
"""

from __future__ import annotations

import io
from dataclasses import dataclass

from reportlab.lib.units import mm
from reportlab.pdfgen import canvas

from .designs import PAPER, Design, cmyk, divider, get_design
from .models import Event, Guest, Options, format_time
from .typeset import (MINCHO, SCRIPT, draw_vertical, fit_size, register_fonts, text_width,
                      to_vertical, vertical_length, wrap)

CARD_SIZE = (105 * mm, 148 * mm)       # A6 縦(洋形2号封筒に封入)
ENVELOPE_SIZE = (114 * mm, 162 * mm)   # 洋形2号 縦長
BLEED = 3 * mm
MARK_AREA = 10 * mm
REGISTRATION = cmyk(1, 1, 1, 1)        # トンボ用レジストレーション
SLUG_TEXT = cmyk(0, 0, 0, 1)


def _new_canvas(buf: io.BytesIO, title: str) -> canvas.Canvas:
    register_fonts()
    c = canvas.Canvas(buf, enforceColorSpace="cmyk", pageCompression=1, pdfVersion=(1, 4),
                      initialFontName=MINCHO)
    c.setTitle(title)
    c.setCreator("Wedding Invitation Maker")
    c.setSubject("印刷入稿用データ (CMYK)")
    return c


# ------------------------------------------------------------------ トンボ

def draw_crop_marks(c, w: float, h: float, bleed: float, length: float) -> None:
    """日本式トンボ(内トンボ/外トンボ + センタートンボ)を描く。原点は仕上がり左下."""
    c.saveState()
    c.setStrokeColor(REGISTRATION)
    c.setLineWidth(0.25)
    b, L = bleed, length
    for cx, sx in ((0, -1), (w, 1)):
        for cy, sy in ((0, -1), (h, 1)):
            far_x, far_y = cx + sx * (b + L), cy + sy * (b + L)
            # 縦線: 仕上がり線と塗り足し線
            c.line(cx, cy + sy * b, cx, far_y)
            c.line(cx + sx * b, cy + sy * b, cx + sx * b, far_y)
            # 横線
            c.line(cx + sx * b, cy, far_x, cy)
            c.line(cx + sx * b, cy + sy * b, far_x, cy + sy * b)
    half = 4 * mm
    for x, y0, y1 in ((w / 2, -b, -b - L), (w / 2, h + b, h + b + L)):
        c.line(x, y0, x, y1)
        c.line(x - half, (y0 + y1) / 2, x + half, (y0 + y1) / 2)
    for y, x0, x1 in ((h / 2, -b, -b - L), (h / 2, w + b, w + b + L)):
        c.line(x0, y, x1, y)
        c.line((x0 + x1) / 2, y - half, (x0 + x1) / 2, y + half)
    c.restoreState()


def _begin_bleed_page(c, w: float, h: float, crop_marks: bool) -> float:
    """塗り足しつきページを準備し、仕上がり左下を原点に移動する。オフセットを返す."""
    off = BLEED + (MARK_AREA if crop_marks else 0)
    c.setPageSize((w + 2 * off, h + 2 * off))
    c.setTrimBox((off, off, off + w, off + h))
    c.setBleedBox((off - BLEED, off - BLEED, off + w + BLEED, off + h + BLEED))
    c.translate(off, off)
    return off


# ------------------------------------------------------------------ 招待状カード

@dataclass
class _Block:
    kind: str           # "text" | "gap" | "divider" | "details"
    lines: list | None = None
    font: str = MINCHO
    size: float = 0
    leading: float = 1.0
    color: object = None
    gap: float = 0


def _details_rows(event: Event) -> list[tuple[str, list[str]]]:
    times = []
    if event.ceremony_time:
        times.append(f"挙式　{format_time(event.ceremony_time)}より")
    if event.party_time:
        times.append(f"披露宴　{format_time(event.party_time)}より")
    if event.reception_open_time:
        times.append(f"（受付　{format_time(event.reception_open_time)}より）")
    venue = [v for v in (event.venue_name, event.venue_address,
                         f"TEL　{event.venue_tel}" if event.venue_tel else "") if v]
    rows = []
    if event.date or times:
        rows.append(("日時", [event.date_text(), *times] if event.date else times))
    if venue:
        rows.append(("会場", venue))
    return rows


def _card_blocks(event: Event, guest: Guest, design: Design, s: float, text_w: float) -> list[_Block]:
    tc = design.text
    body = 7.2 * s
    blocks = [
        _Block("text", [guest.card_line()], size=10.5 * s, leading=1.4, color=tc),
        _Block("gap", gap=5 * mm * s),
        _Block("text", wrap(event.rendered_message(), MINCHO, body, text_w), size=body, leading=1.85, color=tc),
        _Block("gap", gap=3 * mm * s),
        _Block("text", [event.rendered_date_line()], size=body, leading=1.6, color=tc),
        _Block("gap", gap=1.5 * mm * s),
        _Block("text", [n for n in (event.groom_name, event.bride_name) if n], size=9.5 * s, leading=1.6, color=tc),
        _Block("gap", gap=3 * mm * s),
        _Block("divider", gap=4 * mm * s),
        _Block("gap", gap=3 * mm * s),
        _Block("details", size=body, leading=1.75, color=tc),
    ]
    if event.rsvp_deadline:
        rsvp = (f"お手数ではございますが　ご都合のほどを{event.rsvp_text()}までに\n"
                "同封のはがきにてお知らせくださいますようお願い申し上げます")
        blocks += [
            _Block("gap", gap=3 * mm * s),
            _Block("text", wrap(rsvp, MINCHO, 6.2 * s, text_w), size=6.2 * s, leading=1.7, color=tc),
        ]
    return blocks


def _details_layout(rows, size, max_w):
    label_w = text_width("会場", MINCHO, size) + size * 1.2
    wrapped = [(label, [ln for v in values for ln in wrap(v, MINCHO, size, max_w - label_w)]) for label, values in rows]
    value_w = max((text_width(ln, MINCHO, size) for _, vals in wrapped for ln in vals), default=0)
    return wrapped, label_w, label_w + value_w


def _block_height(b: _Block, event: Event, max_w: float) -> float:
    if b.kind == "gap":
        return b.gap
    if b.kind == "divider":
        return b.gap
    if b.kind == "details":
        wrapped, _, _ = _details_layout(_details_rows(event), b.size, max_w)
        return sum(len(v) for _, v in wrapped) * b.size * b.leading
    return len(b.lines) * b.size * b.leading


def _draw_card(c, event: Event, guest: Guest, design: Design) -> None:
    w, h = CARD_SIZE
    design.draw_card(c, design, w, h, BLEED)

    # タイトル
    if design.title_on_band:
        c.setFillColor(PAPER)  # 紙白の抜き文字
        c.setFont(SCRIPT, 26)
        c.drawCentredString(w / 2, h - 16 * mm, design.title)
        top = h - 30 * mm
    else:
        c.setFillColor(design.accent)
        c.setFont(SCRIPT, 26)
        c.drawCentredString(w / 2, h - 21 * mm, design.title)
        divider(c, w / 2, h - 26 * mm, 30 * mm, design.accent)
        top = h - 32 * mm
    bottom = design.bottom_margin
    text_w = w - 26 * mm
    avail = top - bottom

    # 収まるまで文字を縮小
    s = 1.0
    while True:
        blocks = _card_blocks(event, guest, design, s, text_w)
        heights = [_block_height(b, event, text_w) for b in blocks]
        if sum(heights) <= avail or s <= 0.6:
            break
        s -= 0.03
    gaps = [b for b in blocks if b.kind == "gap"]
    extra = max(0.0, avail - sum(heights))
    total_gap = sum(b.gap for b in gaps) or 1
    extra = min(extra, total_gap * 1.2)  # 余白を広げすぎない
    y = top - (avail - sum(heights) - extra) / 2

    for b, bh in zip(blocks, heights):
        if b.kind == "gap":
            y -= b.gap + extra * b.gap / total_gap
            continue
        if b.kind == "divider":
            divider(c, w / 2, y - b.gap / 2, 22 * mm, design.accent)
            y -= b.gap
            continue
        c.setFillColor(b.color)
        c.setFont(b.font, b.size)
        lh = b.size * b.leading
        if b.kind == "details":
            wrapped, label_w, block_w = _details_layout(_details_rows(event), b.size, text_w)
            x0 = (w - block_w) / 2
            for label, values in wrapped:
                for i, line in enumerate(values):
                    base = y - lh + (lh - b.size) / 2 + b.size * 0.12
                    if i == 0:
                        c.drawString(x0, base, label)
                    c.drawString(x0 + label_w, base, line)
                    y -= lh
            continue
        for line in b.lines:
            base = y - lh + (lh - b.size) / 2 + b.size * 0.12
            c.drawCentredString(w / 2, base, line)
            y -= lh


def _slug(c, text: str, w: float) -> None:
    """トンボ外の余白に管理用の情報を入れる."""
    c.setFillColor(SLUG_TEXT)
    c.setFont(MINCHO, 5)
    c.drawString(-BLEED - MARK_AREA + 1 * mm, -BLEED - MARK_AREA + 1.5 * mm, text)


def render_cards(event: Event, guests: list[Guest], design_key: str, options: Options) -> bytes:
    design = get_design(design_key)
    buf = io.BytesIO()
    c = _new_canvas(buf, f"招待状カード ({design.name})")
    w, h = CARD_SIZE
    for i, guest in enumerate(guests, 1):
        _begin_bleed_page(c, w, h, options.crop_marks)
        _draw_card(c, event, guest, design)
        if options.crop_marks:
            draw_crop_marks(c, w, h, BLEED, MARK_AREA - 2 * mm)
            _slug(c, f"招待状 {design.name} 105×148mm 塗り足し3mm CMYK  No.{i} {guest.name} {guest.honorific}", w)
        c.showPage()
    c.save()
    return buf.getvalue()


# ------------------------------------------------------------------ 封筒

def _name_columns(guest: Guest) -> tuple[list[tuple[str, float]], str, float]:
    """縦書き宛名の各列 (テキスト, 開始位置 em)、敬称、敬称の位置 em を返す.

    連名でも敬称の高さを揃える。
    """
    family, given = guest.family_given
    hon = to_vertical(guest.honorific)
    family, given = to_vertical(family), to_vertical(given)
    joint = to_vertical(guest.joint_name.replace("　", " "))
    gap = 0.5
    main_body = family + (" " + given if given else "")
    cols = [(main_body, 0.0)]
    if joint:
        if " " in joint:  # フルネームの連名
            cols.append((joint, 0.0))
        else:  # 名のみ: 主たる名前の「名」に頭を揃える
            start = vertical_length(family) + gap if given else 0.0
            cols.append((joint, start))
    hon_pos = max(start + vertical_length(text) for text, start in cols) + 0.6
    return cols, hon, hon_pos


def _draw_envelope_front(c, guest: Guest, design: Design) -> None:
    w, h = ENVELOPE_SIZE
    design.draw_envelope(c, design, w, h)
    ink = design.text

    # 郵便番号(横書き、右上)
    c.setFillColor(ink)
    if guest.postal_code:
        c.setFont(MINCHO, 13)
        c.drawRightString(w - 12 * mm, h - 17 * mm, f"〒{guest.display_postal}")

    # 住所(右端から縦書き)
    addr_top = h - 26 * mm
    addr_bottom = 30 * mm
    a1 = to_vertical(guest.address1)
    a2 = to_vertical(guest.address2)
    size = min(fit_size(a1, 13, addr_top - addr_bottom),
               fit_size(a2, 13, addr_top - addr_bottom - 26) if a2 else 13)
    x = w - 18 * mm
    draw_vertical(c, a1, x, addr_top, MINCHO, size)
    if a2:
        x2 = x - size * 1.5
        # 建物名は少し下げて、下端を住所の下端付近に揃える
        top2 = max(addr_top - size * 2, addr_bottom + vertical_length(a2) * size)
        top2 = min(top2, addr_top - size * 2)
        draw_vertical(c, a2, x2, top2, MINCHO, size)

    # 宛名(中央、縦書き)
    cols, hon, hon_pos = _name_columns(guest)
    name_top = h - 34 * mm
    name_bottom = 22 * mm
    total = hon_pos + vertical_length(hon)
    tracking = 0.12
    nsize = min(32, (name_top - name_bottom) / (total * (1 + tracking)))
    step = 1 + tracking
    col_gap = nsize * 1.45
    n = len(cols)
    xs = [w / 2 + col_gap * (n - 1) / 2 - col_gap * i for i in range(n)]
    for (text, start), cx in zip(cols, xs):
        draw_vertical(c, text, cx, name_top - start * nsize * step, MINCHO, nsize, tracking)
        draw_vertical(c, hon, cx, name_top - hon_pos * nsize * step, MINCHO, nsize, tracking)


def _draw_envelope_back(c, event: Event, design: Design) -> None:
    w, h = ENVELOPE_SIZE
    ink = design.text
    c.setFillColor(ink)
    bottom = 22 * mm
    addr = to_vertical(event.sender_address)
    names = [to_vertical(n) for n in (event.groom_name, event.bride_name) if n]
    max_len = 95 * mm
    size = min(fit_size(addr, 10, max_len), 10)
    nsize = min([fit_size(n, 13, max_len, 0.1) for n in names] or [13])
    x = w * 0.44
    draw_vertical(c, addr, x, bottom + vertical_length(addr) * size, MINCHO, size)
    x -= size * 1.3 + nsize * 0.6
    for name in names:
        top = bottom + vertical_length(name, 0.1) * nsize
        draw_vertical(c, name, x, top, MINCHO, nsize, 0.1)
        x -= nsize * 1.5
    if event.sender_postal:
        c.setFont(MINCHO, 9)
        c.drawCentredString(w * 0.36, bottom - 7 * mm, f"〒{event.sender_postal}")


def render_envelopes(event: Event, guests: list[Guest], design_key: str, options: Options) -> bytes:
    design = get_design(design_key)
    buf = io.BytesIO()
    c = _new_canvas(buf, f"封筒 宛名 ({design.name})")
    w, h = ENVELOPE_SIZE
    for guest in guests:
        c.setPageSize((w, h))
        c.setTrimBox((0, 0, w, h))
        c.setBleedBox((0, 0, w, h))
        _draw_envelope_front(c, guest, design)
        c.showPage()
    if options.envelope_back and (event.sender_address or event.groom_name or event.bride_name):
        c.setPageSize((w, h))
        _draw_envelope_back(c, event, design)
        c.showPage()
    c.save()
    return buf.getvalue()
