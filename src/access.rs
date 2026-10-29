//! 案内図(ハガキ 100×148mm)のレイアウト.
//!
//! 上から、タイトル・会場 → 地図 → 交通手段ごとの行き方(アイコン付き)→ 最寄り駅からの道順写真 → 補足。
//! 写真の枚数や行の数に合わせて、地図と写真の大きさを自動で決める。

use crate::canvas::{Canvas, Cmyk, Page, Path, MM};
use crate::designs::{self, divider, fill_background, Design};
use crate::fonts::{self, FontId};
use crate::layout::{bleed_page, draw_crop_marks, BLEED, MARK_AREA};
use crate::model::{Access, Event, Options, ACCESS_MODES};
use crate::photos;
use crate::typeset::wrap;

/// 郵便はがき
pub const POSTCARD_SIZE: (f32, f32) = (100.0 * MM, 148.0 * MM);

const MINCHO: FontId = FontId::Mincho;
const MARGIN: f32 = 8.0 * MM;
const PLACEHOLDER: Cmyk = Cmyk(0.0, 0.0, 0.0, 0.08);

// ------------------------------------------------------------------ アイコン

/// 交通手段のアイコン。(cx, cy) を中心に直径 d の円を bg で塗り、記号を fg で描く.
/// 記号は -5〜5 の座標で描いてから縮小する。
pub fn draw_icon(c: &mut Canvas, mode: &str, cx: f32, cy: f32, d: f32, bg: Cmyk, fg: Cmyk) {
    c.save();
    c.set_fill(bg);
    c.circle(cx, cy, d / 2.0, false, true);
    c.translate(cx, cy);
    c.scale(d / 13.0, d / 13.0);
    c.set_fill(fg);
    c.set_stroke(fg);
    let poly = |c: &mut Canvas, pts: &[(f32, f32)]| {
        let mut p = Path::new();
        p.move_to(pts[0].0, pts[0].1);
        for &(x, y) in &pts[1..] {
            p.line_to(x, y);
        }
        p.close();
        c.draw_path(p, false, true);
    };
    match mode {
        "train" | "subway" => {
            let mut p = Path::new();
            p.move_to(-2.6, -2.2)
                .line_to(2.6, -2.2)
                .line_to(2.6, 2.3)
                .curve_to(2.6, 3.6, -2.6, 3.6, -2.6, 2.3)
                .close();
            c.draw_path(p, false, true);
            c.set_fill(bg);
            if mode == "subway" {
                // 地下鉄は窓を 2 つに分ける
                c.rect(-1.9, 0.4, 1.6, 1.8, false, true);
                c.rect(0.3, 0.4, 1.6, 1.8, false, true);
            } else {
                c.rect(-1.9, 0.4, 3.8, 1.8, false, true);
            }
            c.circle(-1.5, -1.1, 0.45, false, true);
            c.circle(1.5, -1.1, 0.45, false, true);
            c.set_line_width(0.6);
            c.line(-1.5, -2.2, -2.6, -3.7);
            c.line(1.5, -2.2, 2.6, -3.7);
        }
        "bus" => {
            c.rect(-3.2, -2.0, 6.4, 5.0, false, true);
            c.rect(-2.7, -3.1, 1.2, 1.2, false, true);
            c.rect(1.5, -3.1, 1.2, 1.2, false, true);
            c.set_fill(bg);
            c.rect(-2.6, 0.5, 5.2, 1.9, false, true);
            c.circle(-2.1, -1.0, 0.4, false, true);
            c.circle(2.1, -1.0, 0.4, false, true);
            c.rect(-1.0, -1.25, 2.0, 0.5, false, true);
        }
        "taxi" | "car" => {
            let mut p = Path::new();
            p.move_to(-4.0, -1.5)
                .line_to(4.0, -1.5)
                .line_to(4.0, 0.2)
                .curve_to(4.0, 0.8, 3.6, 1.0, 3.0, 1.0)
                .line_to(1.9, 1.0)
                .line_to(1.0, 2.6)
                .line_to(-1.8, 2.6)
                .line_to(-2.9, 1.0)
                .line_to(-3.4, 1.0)
                .curve_to(-3.9, 1.0, -4.0, 0.6, -4.0, 0.2)
                .close();
            c.draw_path(p, false, true);
            if mode == "taxi" {
                c.rect(-1.25, 2.85, 1.7, 0.8, false, true);
            }
            c.circle(-2.3, -1.5, 1.05, false, true);
            c.circle(2.3, -1.5, 1.05, false, true);
            c.set_fill(bg);
            poly(c, &[(-2.3, 1.0), (-1.55, 2.1), (-0.55, 2.1), (-0.55, 1.0)]);
            poly(c, &[(-0.05, 1.0), (-0.05, 2.1), (0.75, 2.1), (1.4, 1.0)]);
            c.circle(-2.3, -1.5, 0.42, false, true);
            c.circle(2.3, -1.5, 0.42, false, true);
        }
        "parking" => {
            c.rect(-1.9, -3.3, 1.3, 6.6, false, true);
            let mut p = Path::new();
            p.move_to(-0.6, 3.3)
                .line_to(0.6, 3.3)
                .curve_to(3.1, 3.3, 3.1, -0.5, 0.6, -0.5)
                .line_to(-0.6, -0.5)
                .line_to(-0.6, 0.75)
                .line_to(0.5, 0.75)
                .curve_to(1.75, 0.75, 1.75, 2.05, 0.5, 2.05)
                .line_to(-0.6, 2.05)
                .close();
            c.draw_path(p, false, true);
        }
        "walk" => {
            c.circle(0.5, 3.1, 0.9, false, true);
            c.set_line_width(0.95);
            c.line(0.3, 2.0, -0.3, -0.6);
            c.line(0.15, 1.3, -1.4, 0.1);
            c.line(0.15, 1.3, 1.5, 0.4);
            c.line(-0.3, -0.6, -1.6, -3.4);
            let mut p = Path::new();
            p.move_to(-0.3, -0.6).line_to(1.0, -1.9).line_to(1.3, -3.4);
            c.draw_path(p, true, false);
        }
        "plane" => {
            c.rotate(-45.0);
            poly(
                c,
                &[
                    (0.0, 4.3),
                    (0.6, 3.3),
                    (0.6, 0.9),
                    (4.0, -0.9),
                    (4.0, -1.7),
                    (0.6, -0.8),
                    (0.6, -2.8),
                    (1.6, -3.6),
                    (1.6, -4.2),
                    (0.0, -3.7),
                    (-1.6, -4.2),
                    (-1.6, -3.6),
                    (-0.6, -2.8),
                    (-0.6, -0.8),
                    (-4.0, -1.7),
                    (-4.0, -0.9),
                    (-0.6, 0.9),
                    (-0.6, 3.3),
                ],
            );
        }
        _ => {
            c.circle(0.0, 0.0, 1.6, false, true);
        }
    }
    c.restore();
}

pub fn mode_label(mode: &str) -> &'static str {
    ACCESS_MODES
        .iter()
        .find(|(k, _)| *k == mode)
        .map(|(_, l)| *l)
        .unwrap_or("")
}

// ------------------------------------------------------------------ 写真

/// 写真を枠に収める(全体が見えるように縮小し、余白は残す)。描いた矩形を返す.
fn draw_contain(c: &mut Canvas, id: &str, x: f32, y: f32, w: f32, h: f32) -> Option<[f32; 4]> {
    let p = photos::get(id)?;
    let ar = p.width as f32 / p.height as f32;
    let (iw, ih) = if ar > w / h { (w, w / ar) } else { (h * ar, h) };
    let (ix, iy) = (x + (w - iw) / 2.0, y + (h - ih) / 2.0);
    c.draw_image(id, ix, iy, iw, ih);
    Some([ix, iy, iw, ih])
}

/// 写真で枠を埋める(はみ出す部分は切り取る).
fn draw_cover(c: &mut Canvas, id: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    let Some(p) = photos::get(id) else { return false };
    let scale = (w / p.width as f32).max(h / p.height as f32);
    let (iw, ih) = (p.width as f32 * scale, p.height as f32 * scale);
    c.save();
    c.clip_rect(x, y, w, h);
    c.draw_image(id, x + (w - iw) / 2.0, y + (h - ih) / 2.0, iw, ih);
    c.restore();
    true
}

// ------------------------------------------------------------------ レイアウト

#[derive(Clone, Copy)]
struct Grid {
    cols: usize,
}

const ROUTE_SIZE: f32 = 6.8;
const ROUTE_LEADING: f32 = 1.5;
const ICON: f32 = 5.4 * MM;
const LABEL_W: f32 = 11.0 * MM;
const CAPTION_SIZE: f32 = 6.4;
const CAPTION_LEADING: f32 = 1.45;
const CAPTION_LINES: usize = 3;
const STEP_GAP_X: f32 = 4.0 * MM;
const STEP_GAP_Y: f32 = 2.5 * MM;
const SECTION_GAP: f32 = 3.5 * MM;
const STEPS_HEADER: f32 = 6.0 * MM;
const MAX_MAP: f32 = 95.0 * MM;
const NOTE_SIZE: f32 = 6.3;

fn route_lines(text: &str, w: f32) -> Vec<String> {
    wrap(text, MINCHO, ROUTE_SIZE, w - (ICON + 1.5 * MM) - LABEL_W)
}

fn route_row_height(lines: usize) -> f32 {
    (lines as f32 * ROUTE_SIZE * ROUTE_LEADING + 1.4 * MM).max(ICON + 1.2 * MM)
}

fn caption_lines(text: &str, w: f32) -> Vec<String> {
    let mut lines = wrap(text, MINCHO, CAPTION_SIZE, w);
    if lines.len() > CAPTION_LINES {
        lines.truncate(CAPTION_LINES);
        if let Some(last) = lines.last_mut() {
            last.pop();
            last.push('…');
        }
    }
    lines
}

/// 道順の写真の枠の大きさ(s は縮小率、縦横比 4:3).
fn step_box(content_w: f32, grid: &Grid, s: f32) -> (f32, f32) {
    let full_w = (content_w - STEP_GAP_X * (grid.cols as f32 - 1.0)) / grid.cols as f32;
    let full_w = if grid.cols == 1 { full_w.min(64.0 * MM) } else { full_w };
    (full_w * s, full_w * 0.75 * s)
}

fn steps_height(access: &Access, content_w: f32, grid: &Grid, s: f32) -> f32 {
    if access.steps.is_empty() {
        return 0.0;
    }
    let (pw, ph) = step_box(content_w, grid, s);
    let mut total = STEPS_HEADER;
    for row in access.steps.chunks(grid.cols) {
        let lines = row
            .iter()
            .map(|st| caption_lines(&st.caption, pw).len())
            .max()
            .unwrap_or(0);
        total += ph + 1.2 * MM + lines as f32 * CAPTION_SIZE * CAPTION_LEADING + STEP_GAP_Y;
    }
    total
}

/// 高さ avail に収まり、写真がいちばん大きくなる列数と縮小率.
fn choose_grid(access: &Access, content_w: f32, avail: f32) -> (Grid, f32) {
    let n = access.steps.len().max(1);
    let mut best = (Grid { cols: n.min(3) }, 0.3, 0.0);
    for cols in 1..=n.min(3) {
        let grid = Grid { cols };
        let mut s: f32 = 1.0;
        while s > 0.3 && steps_height(access, content_w, &grid, s) > avail {
            s -= 0.02;
        }
        let pw = step_box(content_w, &grid, s).0;
        if pw > best.2 + 0.5 * MM {
            best = (grid, s, pw);
        }
    }
    (best.0, best.1)
}

/// 背景・枠・タイトル・会場。本文を始める y を返す.
fn draw_header(c: &mut Canvas, event: &Event, access: &Access, d: &Design, subtitle: &str) -> f32 {
    let (w, h) = POSTCARD_SIZE;
    let content_w = w - 2.0 * MARGIN;
    c.save();
    c.clip_rect(-BLEED, -BLEED, w + 2.0 * BLEED, h + 2.0 * BLEED);
    fill_background(c, d, w, h, BLEED);
    c.set_stroke(d.accent);
    c.set_line_width(0.4);
    c.rect(4.5 * MM, 4.5 * MM, w - 9.0 * MM, h - 9.0 * MM, true, false);
    c.restore();

    c.set_fill(d.accent);
    c.set_font(FontId::Script, 22.0);
    c.draw_centred(w / 2.0, h - 15.5 * MM, "Access");
    c.set_fill(d.text);
    c.set_font(MINCHO, 8.0);
    let title = if subtitle.is_empty() {
        access.title.as_str()
    } else {
        subtitle
    };
    c.draw_centred(w / 2.0, h - 21.0 * MM, title);
    divider(c, w / 2.0, h - 24.0 * MM, 24.0 * MM, d.accent);
    c.set_fill(d.text);
    let mut y = h - 26.5 * MM;
    if !event.venue_name.is_empty() {
        c.set_font(MINCHO, 9.0);
        for line in wrap(&event.venue_name, MINCHO, 9.0, content_w) {
            c.draw_centred(w / 2.0, y - 9.0 * 0.9, &line);
            y -= 9.0 * 1.45;
        }
    }
    let mut info = event.venue_address.clone();
    if !event.venue_tel.is_empty() {
        info = format!("{info}　TEL {}", event.venue_tel).trim().to_string();
    }
    if !info.is_empty() {
        c.set_font(MINCHO, 6.8);
        for line in wrap(&info, MINCHO, 6.8, content_w) {
            c.draw_centred(w / 2.0, y - 6.8 * 0.9, &line);
            y -= 6.8 * 1.5;
        }
    }
    y - 2.5 * MM
}

fn draw_routes(c: &mut Canvas, routes: &[(&str, Vec<String>)], d: &Design, mut y: f32) -> f32 {
    for (mode, lines) in routes {
        let row_h = route_row_height(lines.len());
        let icon_cy = y - row_h / 2.0;
        draw_icon(c, mode, MARGIN + ICON / 2.0, icon_cy, ICON, d.accent, Cmyk::PAPER);
        let lh = ROUTE_SIZE * ROUTE_LEADING;
        let text_top = icon_cy + lines.len() as f32 * lh / 2.0;
        let x = MARGIN + ICON + 1.5 * MM;
        let base = |i: usize| text_top - lh * (i as f32 + 1.0) + (lh - ROUTE_SIZE) / 2.0 + ROUTE_SIZE * 0.12;
        c.set_font(MINCHO, ROUTE_SIZE);
        c.set_fill(d.accent);
        c.draw_string(x, base(0), mode_label(mode));
        c.set_fill(d.text);
        for (i, line) in lines.iter().enumerate() {
            c.draw_string(x + LABEL_W, base(i), line);
        }
        y -= row_h;
    }
    y
}

fn draw_steps(c: &mut Canvas, access: &Access, d: &Design, mut y: f32, grid: &Grid, s: f32) {
    let (w, _) = POSTCARD_SIZE;
    let content_w = w - 2.0 * MARGIN;
    c.set_fill(d.accent);
    c.set_font(MINCHO, 7.2);
    let heading = "最寄り駅からの道順";
    let hw = fonts::text_width(MINCHO, heading, 7.2);
    c.draw_string(MARGIN, y - 7.2, heading);
    c.set_stroke(d.sub);
    c.set_line_width(0.35);
    c.line(MARGIN + hw + 2.0 * MM, y - 7.2 * 0.65, w - MARGIN, y - 7.2 * 0.65);
    y -= STEPS_HEADER;

    let (pw, ph) = step_box(content_w, grid, s);
    let lh = CAPTION_SIZE * CAPTION_LEADING;
    let mut number = 0;
    for row in access.steps.chunks(grid.cols) {
        let row_w = pw * row.len() as f32 + STEP_GAP_X * (row.len() as f32 - 1.0);
        let x0 = (w - row_w) / 2.0;
        let mut caption_h: f32 = 0.0;
        for (i, step) in row.iter().enumerate() {
            number += 1;
            let x = x0 + i as f32 * (pw + STEP_GAP_X);
            if !draw_cover(c, &step.photo, x, y - ph, pw, ph) {
                c.set_fill(PLACEHOLDER);
                c.rect(x, y - ph, pw, ph, false, true);
            }
            // 番号
            let r = (2.3 * MM).min(pw * 0.12);
            let (bx, by) = (x + r + 1.0 * MM, y - r - 1.0 * MM);
            c.set_fill(d.accent);
            c.circle(bx, by, r, false, true);
            c.set_fill(Cmyk::PAPER);
            let size = r * 1.15;
            c.set_font(MINCHO, size);
            c.draw_centred(bx, by - size * 0.36, &number.to_string());
            // 次の写真への矢印
            let is_last = number == access.steps.len();
            if !is_last {
                let a = 1.3 * MM;
                let mut p = Path::new();
                if i + 1 < row.len() {
                    let (ax, ay) = (x + pw + STEP_GAP_X / 2.0, y - ph / 2.0);
                    p.move_to(ax - a * 0.5, ay + a)
                        .line_to(ax + a * 0.7, ay)
                        .line_to(ax - a * 0.5, ay - a);
                } else {
                    // 行の終わりは右下に下向きの矢印
                    let (ax, ay) = (x + pw - a * 1.2, y - ph - 0.6 * MM);
                    p.move_to(ax - a, ay + a * 0.2)
                        .line_to(ax + a, ay + a * 0.2)
                        .line_to(ax, ay - a);
                }
                p.close();
                c.set_fill(d.accent);
                c.draw_path(p, false, true);
            }
            // 説明
            let lines = caption_lines(&step.caption, if is_last { pw } else { pw - 3.0 * MM });
            c.set_fill(d.text);
            c.set_font(MINCHO, CAPTION_SIZE);
            for (j, line) in lines.iter().enumerate() {
                let base = y - ph - 1.2 * MM - lh * (j as f32 + 1.0) + (lh - CAPTION_SIZE) / 2.0 + CAPTION_SIZE * 0.12;
                c.draw_string(x, base, line);
            }
            caption_h = caption_h.max(lines.len() as f32 * lh);
        }
        y -= ph + 1.2 * MM + caption_h + STEP_GAP_Y;
    }
}

fn draw_note(c: &mut Canvas, lines: &[String], d: &Design) {
    c.set_fill(d.text);
    c.set_font(MINCHO, NOTE_SIZE);
    let lh = NOTE_SIZE * 1.6;
    let mut y = MARGIN + lines.len() as f32 * lh;
    for line in lines {
        c.draw_string(MARGIN, y - lh + NOTE_SIZE * 0.42, line);
        y -= lh;
    }
}

/// 地図と道順写真の両方があれば表裏 2 ページ(表: 地図・交通手段・補足、裏: 道順写真)にする.
pub fn is_two_sided(access: &Access) -> bool {
    photos::has(&access.map_photo) && !access.steps.is_empty()
}

fn page(options: &Options, d: &Design, side: &str, draw: impl FnOnce(&mut Canvas)) -> Page {
    let (w, h) = POSTCARD_SIZE;
    let mut c = bleed_page(w, h, options.crop_marks);
    draw(&mut c);
    if options.crop_marks {
        draw_crop_marks(&mut c, w, h, BLEED, MARK_AREA - 2.0 * MM);
        c.set_fill(Cmyk::BLACK);
        c.set_font(MINCHO, 5.0);
        let slug = format!("案内図{side} {} ハガキ 100×148mm 塗り足し3mm CMYK", d.name);
        c.draw_string(-BLEED - MARK_AREA + 1.0 * MM, -BLEED - MARK_AREA + 1.5 * MM, &slug);
    }
    c.finish()
}

/// 案内図のページ(1 枚、または表裏 2 枚).
pub fn access_pages(event: &Event, access: &Access, design: &str, options: &Options) -> Vec<Page> {
    let d = designs::get(design);
    let (w, _) = POSTCARD_SIZE;
    let content_w = w - 2.0 * MARGIN;
    let has_map = photos::has(&access.map_photo);
    let two_sided = is_two_sided(access);
    let note_lines = if access.note.is_empty() {
        Vec::new()
    } else {
        wrap(&access.note, MINCHO, NOTE_SIZE, content_w)
    };
    let note_h = note_lines.len() as f32 * NOTE_SIZE * 1.6;
    let bottom = MARGIN + if note_h > 0.0 { note_h + 2.0 * MM } else { 0.0 };
    let routes: Vec<(&str, Vec<String>)> = access
        .routes
        .iter()
        .map(|r| (r.mode.as_str(), route_lines(&r.text, content_w)))
        .collect();
    let routes_h: f32 = routes.iter().map(|(_, l)| route_row_height(l.len())).sum();

    let front = page(options, d, if two_sided { "(表)" } else { "" }, |c| {
        let mut y = draw_header(c, event, access, d, "");
        let with_steps = !two_sided && !access.steps.is_empty();
        let sections = [has_map, !routes.is_empty(), with_steps].iter().filter(|b| **b).count() as f32;
        let rest = (y - bottom - routes_h - SECTION_GAP * (sections - 1.0).max(0.0)).max(0.0);
        let (grid, s) = choose_grid(access, content_w, rest);
        let steps_h = if with_steps {
            steps_height(access, content_w, &grid, s)
        } else {
            0.0
        };
        let map_h = if has_map {
            (rest - steps_h).clamp(0.0, MAX_MAP)
        } else {
            0.0
        };
        if has_map && map_h > 0.0 {
            if let Some([ix, iy, iw, ih]) = draw_contain(c, &access.map_photo, MARGIN, y - map_h, content_w, map_h) {
                c.set_stroke(d.sub);
                c.set_line_width(0.4);
                c.rect(ix, iy, iw, ih, true, false);
            }
            y -= map_h + SECTION_GAP;
        }
        if !routes.is_empty() {
            y = draw_routes(c, &routes, d, y) - SECTION_GAP;
        }
        if with_steps {
            draw_steps(c, access, d, y, &grid, s);
        }
        draw_note(c, &note_lines, d);
    });
    if !two_sided {
        return vec![front];
    }
    let back = page(options, d, "(裏)", |c| {
        let y = draw_header(c, event, access, d, "");
        let (grid, s) = choose_grid(access, content_w, y - MARGIN);
        // 余白が残るときは上下の中央に寄せる
        let spare = (y - MARGIN - steps_height(access, content_w, &grid, s)).max(0.0);
        draw_steps(c, access, d, y - (spare / 2.0).min(12.0 * MM), &grid, s);
    });
    vec![front, back]
}
