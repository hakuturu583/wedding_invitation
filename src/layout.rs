//! 招待状カードと封筒のレイアウト.
//!
//! - 招待状カードは塗り足し 3mm + 日本式トンボ、TrimBox/BleedBox を設定
//! - 封筒は既製封筒への印刷を想定し仕上がりサイズ、宛名は縦書き

use crate::canvas::{Canvas, Cmyk, Page, MM};
use crate::designs::{self, divider, Design};
use crate::fonts::{self, FontId};
use crate::model::{format_time, Event, Guest, Options};
use crate::typeset::{draw_vertical, fit_size, to_vertical, vertical_length, wrap};

/// A6 縦(洋形 2 号封筒に封入)
pub const CARD_SIZE: (f32, f32) = (105.0 * MM, 148.0 * MM);
/// 洋形 2 号 縦長
pub const ENVELOPE_SIZE: (f32, f32) = (114.0 * MM, 162.0 * MM);
pub const BLEED: f32 = 3.0 * MM;
pub const MARK_AREA: f32 = 10.0 * MM;

const MINCHO: FontId = FontId::Mincho;

// ------------------------------------------------------------------ トンボ

/// 日本式トンボ(内トンボ/外トンボ + センタートンボ)。原点は仕上がり左下.
pub fn draw_crop_marks(c: &mut Canvas, w: f32, h: f32, b: f32, len: f32) {
    c.save();
    c.set_stroke(Cmyk::REGISTRATION);
    c.set_line_width(0.25);
    for (cx, sx) in [(0.0, -1.0), (w, 1.0)] {
        for (cy, sy) in [(0.0, -1.0), (h, 1.0)] {
            let far_x = cx + sx * (b + len);
            let far_y = cy + sy * (b + len);
            // 縦線: 仕上がり線と塗り足し線
            c.line(cx, cy + sy * b, cx, far_y);
            c.line(cx + sx * b, cy + sy * b, cx + sx * b, far_y);
            // 横線
            c.line(cx + sx * b, cy, far_x, cy);
            c.line(cx + sx * b, cy + sy * b, far_x, cy + sy * b);
        }
    }
    let half = 4.0 * MM;
    for (x, y0, y1) in [(w / 2.0, -b, -b - len), (w / 2.0, h + b, h + b + len)] {
        c.line(x, y0, x, y1);
        c.line(x - half, (y0 + y1) / 2.0, x + half, (y0 + y1) / 2.0);
    }
    for (y, x0, x1) in [(h / 2.0, -b, -b - len), (h / 2.0, w + b, w + b + len)] {
        c.line(x0, y, x1, y);
        c.line((x0 + x1) / 2.0, y - half, (x0 + x1) / 2.0, y + half);
    }
    c.restore();
}

/// 塗り足しつきページを用意し、仕上がり左下を原点に移動する.
fn bleed_page(w: f32, h: f32, crop_marks: bool) -> Canvas {
    let off = BLEED + if crop_marks { MARK_AREA } else { 0.0 };
    let mut c = Canvas::new(
        w + 2.0 * off,
        h + 2.0 * off,
        [off, off, off + w, off + h],
        [off - BLEED, off - BLEED, off + w + BLEED, off + h + BLEED],
    );
    c.translate(off, off);
    c
}

// ------------------------------------------------------------------ 招待状カード

enum Block {
    Text {
        lines: Vec<String>,
        size: f32,
        leading: f32,
    },
    Gap(f32),
    Divider(f32),
    Details {
        size: f32,
        leading: f32,
    },
}

fn details_rows(event: &Event) -> Vec<(&'static str, Vec<String>)> {
    let mut times = Vec::new();
    if !event.ceremony_time.is_empty() {
        times.push(format!("挙式　{}より", format_time(&event.ceremony_time)));
    }
    if !event.party_time.is_empty() {
        times.push(format!("披露宴　{}より", format_time(&event.party_time)));
    }
    if !event.reception_open_time.is_empty() {
        times.push(format!("（受付　{}より）", format_time(&event.reception_open_time)));
    }
    let mut rows = Vec::new();
    if !event.date.is_empty() || !times.is_empty() {
        let mut values = Vec::new();
        if !event.date.is_empty() {
            values.push(event.date_text());
        }
        values.extend(times);
        rows.push(("日時", values));
    }
    let mut venue: Vec<String> = [&event.venue_name, &event.venue_address]
        .into_iter()
        .filter(|v| !v.is_empty())
        .cloned()
        .collect();
    if !event.venue_tel.is_empty() {
        venue.push(format!("TEL　{}", event.venue_tel));
    }
    if !venue.is_empty() {
        rows.push(("会場", venue));
    }
    rows
}

struct DetailsLayout {
    rows: Vec<(&'static str, Vec<String>)>,
    label_w: f32,
    block_w: f32,
}

fn details_layout(event: &Event, size: f32, max_w: f32) -> DetailsLayout {
    let label_w = fonts::text_width(MINCHO, "会場", size) + size * 1.2;
    let rows: Vec<_> = details_rows(event)
        .into_iter()
        .map(|(label, values)| {
            let lines: Vec<String> = values
                .iter()
                .flat_map(|v| wrap(v, MINCHO, size, max_w - label_w))
                .collect();
            (label, lines)
        })
        .collect();
    let value_w = rows
        .iter()
        .flat_map(|(_, lines)| lines.iter())
        .map(|l| fonts::text_width(MINCHO, l, size))
        .fold(0.0, f32::max);
    DetailsLayout {
        rows,
        label_w,
        block_w: label_w + value_w,
    }
}

fn card_blocks(event: &Event, guest: &Guest, s: f32, text_w: f32) -> Vec<Block> {
    let body = 7.2 * s;
    let names: Vec<String> = [&event.groom_name, &event.bride_name]
        .into_iter()
        .filter(|n| !n.is_empty())
        .cloned()
        .collect();
    let mut blocks = vec![
        Block::Text {
            lines: vec![guest.card_line()],
            size: 10.5 * s,
            leading: 1.4,
        },
        Block::Gap(5.0 * MM * s),
        Block::Text {
            lines: wrap(&event.rendered_message(), MINCHO, body, text_w),
            size: body,
            leading: 1.85,
        },
        Block::Gap(3.0 * MM * s),
        Block::Text {
            lines: vec![event.rendered_date_line()],
            size: body,
            leading: 1.6,
        },
        Block::Gap(1.5 * MM * s),
        Block::Text {
            lines: names,
            size: 9.5 * s,
            leading: 1.6,
        },
        Block::Gap(3.0 * MM * s),
        Block::Divider(4.0 * MM * s),
        Block::Gap(3.0 * MM * s),
        Block::Details {
            size: body,
            leading: 1.75,
        },
    ];
    if !event.rsvp_deadline.is_empty() {
        let rsvp = format!(
            "お手数ではございますが　ご都合のほどを{}までに\n同封のはがきにてお知らせくださいますようお願い申し上げます",
            event.rsvp_text()
        );
        let size = 6.2 * s;
        blocks.push(Block::Gap(3.0 * MM * s));
        blocks.push(Block::Text {
            lines: wrap(&rsvp, MINCHO, size, text_w),
            size,
            leading: 1.7,
        });
    }
    blocks
}

fn block_height(b: &Block, event: &Event, max_w: f32) -> f32 {
    match b {
        Block::Gap(g) | Block::Divider(g) => *g,
        Block::Text { lines, size, leading } => lines.len() as f32 * size * leading,
        Block::Details { size, leading } => {
            let n: usize = details_layout(event, *size, max_w)
                .rows
                .iter()
                .map(|(_, l)| l.len())
                .sum();
            n as f32 * size * leading
        }
    }
}

/// 行の高さ lh の中で文字を上下中央に置くベースライン.
fn baseline(top: f32, lh: f32, size: f32) -> f32 {
    top - lh + (lh - size) / 2.0 + size * 0.12
}

fn draw_card(c: &mut Canvas, event: &Event, guest: &Guest, d: &Design) {
    let (w, h) = CARD_SIZE;
    // 装飾は塗り足しの外にはみ出さないよう切り抜く
    c.save();
    c.clip_rect(-BLEED, -BLEED, w + 2.0 * BLEED, h + 2.0 * BLEED);
    (d.draw_card)(c, d, w, h, BLEED);
    c.restore();

    // タイトル
    c.set_font(FontId::Script, 26.0);
    let top = if d.title_on_band {
        c.set_fill(Cmyk::PAPER); // 紙白の抜き文字
        c.draw_centred(w / 2.0, h - 16.0 * MM, d.title);
        h - 30.0 * MM
    } else {
        c.set_fill(d.accent);
        c.draw_centred(w / 2.0, h - 21.0 * MM, d.title);
        divider(c, w / 2.0, h - 26.0 * MM, 30.0 * MM, d.accent);
        h - 32.0 * MM
    };
    let text_w = w - 26.0 * MM;
    let avail = top - d.bottom_margin;

    // 収まるまで文字を縮小
    let mut s = 1.0;
    let (blocks, heights) = loop {
        let blocks = card_blocks(event, guest, s, text_w);
        let heights: Vec<f32> = blocks.iter().map(|b| block_height(b, event, text_w)).collect();
        if heights.iter().sum::<f32>() <= avail || s <= 0.6 {
            break (blocks, heights);
        }
        s -= 0.03;
    };
    let used: f32 = heights.iter().sum();
    let total_gap: f32 = blocks
        .iter()
        .map(|b| if let Block::Gap(g) = b { *g } else { 0.0 })
        .sum::<f32>()
        .max(1.0);
    let extra = (avail - used).max(0.0).min(total_gap * 1.2); // 余白を広げすぎない
    let mut y = top - (avail - used - extra).max(0.0) / 2.0;

    c.set_fill(d.text);
    for b in &blocks {
        match b {
            Block::Gap(g) => y -= g + extra * g / total_gap,
            Block::Divider(g) => {
                divider(c, w / 2.0, y - g / 2.0, 22.0 * MM, d.accent);
                c.set_fill(d.text);
                y -= g;
            }
            Block::Text { lines, size, leading } => {
                c.set_font(MINCHO, *size);
                let lh = size * leading;
                for line in lines {
                    c.draw_centred(w / 2.0, baseline(y, lh, *size), line);
                    y -= lh;
                }
            }
            Block::Details { size, leading } => {
                let layout = details_layout(event, *size, text_w);
                c.set_font(MINCHO, *size);
                let lh = size * leading;
                let x0 = (w - layout.block_w) / 2.0;
                for (label, lines) in &layout.rows {
                    for (i, line) in lines.iter().enumerate() {
                        let base = baseline(y, lh, *size);
                        if i == 0 {
                            c.draw_string(x0, base, label);
                        }
                        c.draw_string(x0 + layout.label_w, base, line);
                        y -= lh;
                    }
                }
            }
        }
    }
}

pub fn card_pages(event: &Event, guests: &[Guest], design: &str, options: &Options) -> Vec<Page> {
    let d = designs::get(design);
    let (w, h) = CARD_SIZE;
    guests
        .iter()
        .enumerate()
        .map(|(i, guest)| {
            let mut c = bleed_page(w, h, options.crop_marks);
            draw_card(&mut c, event, guest, d);
            if options.crop_marks {
                draw_crop_marks(&mut c, w, h, BLEED, MARK_AREA - 2.0 * MM);
                // トンボ外の余白に管理用の情報を入れる
                c.set_fill(Cmyk::BLACK);
                c.set_font(MINCHO, 5.0);
                let slug = format!(
                    "招待状 {} 105×148mm 塗り足し3mm CMYK  No.{} {} {}",
                    d.name,
                    i + 1,
                    guest.name,
                    guest.honorific
                );
                c.draw_string(-BLEED - MARK_AREA + 1.0 * MM, -BLEED - MARK_AREA + 1.5 * MM, &slug);
            }
            c.finish()
        })
        .collect()
}

// ------------------------------------------------------------------ 封筒

/// 縦書き宛名の各列 (テキスト, 開始位置 em)、敬称、敬称の位置 em.
/// 連名でも敬称の高さを揃える。
fn name_columns(guest: &Guest) -> (Vec<(String, f32)>, String, f32) {
    let (family, given) = guest.family_given();
    let (family, given) = (to_vertical(&family), to_vertical(&given));
    let honorific = to_vertical(&guest.honorific);
    let joint = to_vertical(&guest.joint_name.replace('　', " "));
    let main = if given.is_empty() {
        family.clone()
    } else {
        format!("{family} {given}")
    };
    let mut cols = vec![(main, 0.0)];
    if !joint.is_empty() {
        if joint.contains(' ') {
            cols.push((joint, 0.0)); // フルネームの連名
        } else {
            // 名のみ: 主たる名前の「名」に頭を揃える
            let start = if given.is_empty() {
                0.0
            } else {
                vertical_length(&family, 0.0) + 0.5
            };
            cols.push((joint, start));
        }
    }
    let hon_pos = cols
        .iter()
        .map(|(t, s)| s + vertical_length(t, 0.0))
        .fold(0.0, f32::max)
        + 0.6;
    (cols, honorific, hon_pos)
}

fn draw_envelope_front(c: &mut Canvas, guest: &Guest, d: &Design) {
    let (w, h) = ENVELOPE_SIZE;
    c.save();
    c.clip_rect(0.0, 0.0, w, h);
    (d.draw_envelope)(c, d, w, h);
    c.restore();
    c.set_fill(d.text);

    // 郵便番号(横書き、右上)
    if !guest.postal_code.is_empty() {
        c.set_font(MINCHO, 13.0);
        c.draw_right(w - 12.0 * MM, h - 17.0 * MM, &format!("〒{}", guest.display_postal()));
    }

    // 住所(右端から縦書き)
    let addr_top = h - 26.0 * MM;
    let addr_bottom = 30.0 * MM;
    let a1 = to_vertical(&guest.address1);
    let a2 = to_vertical(&guest.address2);
    let mut size = fit_size(&a1, 13.0, addr_top - addr_bottom, 0.0);
    if !a2.is_empty() {
        size = size.min(fit_size(&a2, 13.0, addr_top - addr_bottom - 26.0, 0.0));
    }
    let x = w - 18.0 * MM;
    draw_vertical(c, &a1, x, addr_top, MINCHO, size, 0.0);
    if !a2.is_empty() {
        // 建物名は 2 文字分下げて始める
        draw_vertical(c, &a2, x - size * 1.5, addr_top - size * 2.0, MINCHO, size, 0.0);
    }

    // 宛名(中央、縦書き)
    let (cols, honorific, hon_pos) = name_columns(guest);
    let name_top = h - 34.0 * MM;
    let name_bottom = 22.0 * MM;
    let tracking = 0.12;
    let step = 1.0 + tracking;
    let total = hon_pos + vertical_length(&honorific, 0.0);
    let nsize = 32.0f32.min((name_top - name_bottom) / (total * step));
    let col_gap = nsize * 1.45;
    let n = cols.len() as f32;
    for (i, (text, start)) in cols.iter().enumerate() {
        let cx = w / 2.0 + col_gap * (n - 1.0) / 2.0 - col_gap * i as f32;
        draw_vertical(c, text, cx, name_top - start * nsize * step, MINCHO, nsize, tracking);
        draw_vertical(
            c,
            &honorific,
            cx,
            name_top - hon_pos * nsize * step,
            MINCHO,
            nsize,
            tracking,
        );
    }
}

fn draw_envelope_back(c: &mut Canvas, event: &Event, d: &Design) {
    let (w, _) = ENVELOPE_SIZE;
    c.set_fill(d.text);
    let bottom = 22.0 * MM;
    let max_len = 95.0 * MM;
    let addr = to_vertical(&event.sender_address);
    let names: Vec<String> = [&event.groom_name, &event.bride_name]
        .into_iter()
        .filter(|n| !n.is_empty())
        .map(|n| to_vertical(n))
        .collect();
    let size = fit_size(&addr, 10.0, max_len, 0.0);
    let nsize = names
        .iter()
        .map(|n| fit_size(n, 13.0, max_len, 0.1))
        .fold(13.0, f32::min);
    let mut x = w * 0.44;
    draw_vertical(
        c,
        &addr,
        x,
        bottom + vertical_length(&addr, 0.0) * size,
        MINCHO,
        size,
        0.0,
    );
    x -= size * 1.3 + nsize * 0.6;
    for name in &names {
        let top = bottom + vertical_length(name, 0.1) * nsize;
        draw_vertical(c, name, x, top, MINCHO, nsize, 0.1);
        x -= nsize * 1.5;
    }
    if !event.sender_postal.is_empty() {
        c.set_font(MINCHO, 9.0);
        c.draw_centred(w * 0.36, bottom - 7.0 * MM, &format!("〒{}", event.sender_postal));
    }
}

fn envelope_canvas() -> Canvas {
    let (w, h) = ENVELOPE_SIZE;
    Canvas::new(w, h, [0.0, 0.0, w, h], [0.0, 0.0, w, h])
}

pub fn envelope_pages(event: &Event, guests: &[Guest], design: &str, options: &Options) -> Vec<Page> {
    let d = designs::get(design);
    let mut pages: Vec<Page> = guests
        .iter()
        .map(|guest| {
            let mut c = envelope_canvas();
            draw_envelope_front(&mut c, guest, d);
            c.finish()
        })
        .collect();
    let has_sender = !(event.sender_address.is_empty() && event.groom_name.is_empty() && event.bride_name.is_empty());
    if options.envelope_back && has_sender {
        let mut c = envelope_canvas();
        draw_envelope_back(&mut c, event, d);
        pages.push(c.finish());
    }
    pages
}
