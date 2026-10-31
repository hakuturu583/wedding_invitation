//! 案内図(ハガキ 100×148mm)のレイアウト.
//!
//! 上から、タイトル・会場 → 地図 → 交通手段ごとの行き方(アイコン付き)→ 最寄り駅からの道順写真 → 補足。
//! 写真の枚数や行の数に合わせて、地図と写真の大きさを自動で決める。
//! 横向き(148×100mm)では見出しを詰め、地図と交通手段を左右に並べる。

use crate::canvas::{Canvas, Cmyk, Page, Path, MM};
use crate::designs::{self, divider, Design};
use crate::fonts::{self, FontId};
use crate::layout::{bleed_page, draw_crop_marks, BLEED, MARK_AREA};
use crate::model::{Access, Event, Options, ACCESS_MODES};
use crate::photos;
use crate::typeset::{wrap, wrap_phrases};

/// 郵便はがき(縦)
pub const POSTCARD_SIZE: (f32, f32) = (100.0 * MM, 148.0 * MM);

/// 仕上がりサイズ(横向きなら幅と高さを入れ替える).
pub fn postcard_size(access: &Access) -> (f32, f32) {
    let (w, h) = POSTCARD_SIZE;
    if access.is_landscape() {
        (h, w)
    } else {
        (w, h)
    }
}

const MINCHO: FontId = FontId::Mincho;
const MARGIN: f32 = 10.0 * MM;
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
pub(crate) fn draw_cover(c: &mut Canvas, id: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
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
/// 交通手段と同じ面に並べるときの、道順写真の最小の幅
const MIN_FRONT_PHOTO: f32 = 30.0 * MM;
const MAX_MAP: f32 = 95.0 * MM;
const NOTE_SIZE: f32 = 6.3;

/// 交通手段の文字・アイコンの大きさ(k は縮小率)と、1 件あたりの最大行数.
#[derive(Clone, Copy)]
struct RouteStyle {
    k: f32,
    max_lines: usize,
}

impl RouteStyle {
    fn size(&self) -> f32 {
        ROUTE_SIZE * self.k
    }
    fn icon(&self) -> f32 {
        ICON * self.k
    }
    fn label_w(&self) -> f32 {
        LABEL_W * self.k
    }
}

type RouteRows<'a> = Vec<(&'a str, Vec<String>)>;

fn route_lines(text: &str, w: f32, st: RouteStyle) -> Vec<String> {
    let mut lines = wrap_phrases(text, MINCHO, st.size(), w - (st.icon() + 1.5 * MM) - st.label_w());
    if lines.len() > st.max_lines {
        lines.truncate(st.max_lines);
        if let Some(last) = lines.last_mut() {
            last.pop();
            last.push('…');
        }
    }
    lines
}

fn route_row_height(lines: usize, st: RouteStyle) -> f32 {
    (lines as f32 * st.size() * ROUTE_LEADING + 1.4 * MM * st.k).max(st.icon() + 1.2 * MM * st.k)
}

/// 高さ limit に収まるよう、文字を縮め(6 割まで)、それでも収まらなければ 1 件あたりの行数を減らす.
fn fit_routes(access: &Access, w: f32, limit: f32) -> (RouteRows<'_>, RouteStyle, f32) {
    let layout = |st: RouteStyle| {
        let rows: RouteRows = access
            .routes
            .iter()
            .map(|r| (r.mode.as_str(), route_lines(&r.text, w, st)))
            .collect();
        let h = rows.iter().map(|(_, l)| route_row_height(l.len(), st)).sum::<f32>();
        (rows, st, h)
    };
    let scales = (0..=8).map(|i| RouteStyle {
        k: 1.0 - 0.05 * i as f32,
        max_lines: usize::MAX,
    });
    let shorter = [2, 1].map(|max_lines| RouteStyle { k: 0.6, max_lines });
    let mut last = None;
    for st in scales.chain(shorter) {
        let result = layout(st);
        if result.2 <= limit {
            return result;
        }
        last = Some(result);
    }
    last.expect("候補は空でない")
}

fn caption_lines(text: &str, w: f32) -> Vec<String> {
    let mut lines = wrap_phrases(text, MINCHO, CAPTION_SIZE, w);
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
/// 説明が省略されない並べ方、最後の行が 1 枚だけにならない並べ方を優先する。
/// どの並べ方でも収まらなければ、いちばん低くなる並べ方を選ぶ。
fn choose_grid(access: &Access, content_w: f32, avail: f32, max_cols: usize) -> (Grid, f32) {
    const MIN_SCALE: f32 = 0.3;
    let n = access.steps.len().max(1);
    // (並べ方, 縮小率, (説明が省略されない, 1 枚だけの行がない, 写真の幅))
    let mut fitting: Option<(Grid, f32, (bool, bool, f32))> = None;
    let mut lowest: Option<(Grid, f32)> = None; // (並べ方, 最小の高さ)
    for cols in 1..=n.min(max_cols) {
        let grid = Grid { cols };
        let mut s: f32 = 1.0;
        while s > MIN_SCALE && steps_height(access, content_w, &grid, s) > avail {
            s -= 0.02;
        }
        let height = steps_height(access, content_w, &grid, s);
        if height <= avail {
            let pw = step_box(content_w, &grid, s).0;
            let whole = access
                .steps
                .iter()
                .all(|st| wrap_phrases(&st.caption, MINCHO, CAPTION_SIZE, pw).len() <= CAPTION_LINES);
            let balanced = cols == 1 || n <= cols || n % cols != 1;
            let rank = (whole, balanced, pw);
            let better = fitting.is_none_or(|(_, _, best)| {
                (rank.0, rank.1) > (best.0, best.1) || ((rank.0, rank.1) == (best.0, best.1) && pw > best.2 + 0.5 * MM)
            });
            if better {
                fitting = Some((grid, s, rank));
            }
        } else if lowest.is_none_or(|(_, h)| height < h) {
            lowest = Some((grid, height));
        }
    }
    match (fitting, lowest) {
        (Some((grid, s, _)), _) => (grid, s),
        (None, Some((grid, _))) => (grid, MIN_SCALE),
        (None, None) => (Grid { cols: 1 }, 1.0),
    }
}

/// 背景・飾り・タイトル・会場。本文を始める y を返す(横向きは見出しを詰める).
fn draw_header(c: &mut Canvas, event: &Event, access: &Access, d: &Design) -> f32 {
    let (w, h) = postcard_size(access);
    let compact = access.is_landscape();
    let content_w = w - 2.0 * MARGIN;
    // 招待状と同じ背景と飾り。塗り足しの外にはみ出さないよう切り抜く
    c.save();
    c.clip_rect(-BLEED, -BLEED, w + 2.0 * BLEED, h + 2.0 * BLEED);
    designs::draw_access(c, d, w, h, BLEED);
    c.restore();

    let k = if compact { 0.8 } else { 1.0 };
    // 帯のあるデザインは帯の上に紙白の抜き文字でタイトルを置く
    let on_band = d.title_on_band;
    c.set_fill(if on_band { Cmyk::PAPER } else { d.accent });
    c.set_font(FontId::Script, 22.0 * k);
    c.draw_centred(w / 2.0, h - if on_band { 12.0 } else { 15.5 } * MM * k, "Access");
    c.set_fill(if on_band { Cmyk::PAPER } else { d.text });
    c.set_font(MINCHO, 8.0 * k.max(0.9));
    c.draw_centred(w / 2.0, h - if on_band { 18.5 } else { 21.0 } * MM * k, &access.title);
    let mut y = if on_band {
        h - designs::MODERN_BAND - 3.0 * MM
    } else {
        divider(c, w / 2.0, h - 24.0 * MM * k, 24.0 * MM, d.accent);
        h - 26.5 * MM * k
    };
    c.set_fill(d.text);
    let name_size = 9.0 * k.max(0.92);
    if !event.venue_name.is_empty() {
        c.set_font(MINCHO, name_size);
        for line in wrap(&event.venue_name, MINCHO, name_size, content_w) {
            c.draw_centred(w / 2.0, y - name_size * 0.9, &line);
            y -= name_size * 1.45;
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
    y - 2.5 * MM * k
}

/// 交通手段を x0 から描く。描いた後の y を返す.
fn draw_routes(c: &mut Canvas, routes: &[(&str, Vec<String>)], st: RouteStyle, d: &Design, x0: f32, mut y: f32) -> f32 {
    let (size, icon) = (st.size(), st.icon());
    for (mode, lines) in routes {
        let row_h = route_row_height(lines.len(), st);
        let icon_cy = y - row_h / 2.0;
        draw_icon(c, mode, x0 + icon / 2.0, icon_cy, icon, d.accent, Cmyk::PAPER);
        let lh = size * ROUTE_LEADING;
        let text_top = icon_cy + lines.len() as f32 * lh / 2.0;
        let x = x0 + icon + 1.5 * MM;
        let base = |i: usize| text_top - lh * (i as f32 + 1.0) + (lh - size) / 2.0 + size * 0.12;
        c.set_font(MINCHO, size);
        c.set_fill(d.accent);
        c.draw_string(x, base(0), mode_label(mode));
        c.set_fill(d.text);
        for (i, line) in lines.iter().enumerate() {
            c.draw_string(x + st.label_w(), base(i), line);
        }
        y -= row_h;
    }
    y
}

/// 道順の写真を x0 から幅 content_w の中に並べる.
#[allow(clippy::too_many_arguments)]
fn draw_steps(c: &mut Canvas, access: &Access, d: &Design, x0: f32, content_w: f32, mut y: f32, grid: &Grid, s: f32) {
    c.set_fill(d.accent);
    c.set_font(MINCHO, 7.2);
    let heading = access.steps_heading();
    let hw = fonts::text_width(MINCHO, heading, 7.2);
    c.draw_string(x0, y - 7.2, heading);
    c.set_stroke(d.sub);
    c.set_line_width(0.35);
    c.line(x0 + hw + 2.0 * MM, y - 7.2 * 0.65, x0 + content_w, y - 7.2 * 0.65);
    y -= STEPS_HEADER;

    let (pw, ph) = step_box(content_w, grid, s);
    let lh = CAPTION_SIZE * CAPTION_LEADING;
    let mut number = 0;
    for row in access.steps.chunks(grid.cols) {
        let row_w = pw * row.len() as f32 + STEP_GAP_X * (row.len() as f32 - 1.0);
        let left = x0 + (content_w - row_w) / 2.0;
        let mut caption_h: f32 = 0.0;
        for (i, step) in row.iter().enumerate() {
            number += 1;
            let x = left + i as f32 * (pw + STEP_GAP_X);
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
            // 同じ行の次の写真への矢印(行の折り返しは番号で順番がわかるので矢印を付けない)
            if i + 1 < row.len() {
                let a = 1.3 * MM;
                let (ax, ay) = (x + pw + STEP_GAP_X / 2.0, y - ph / 2.0);
                let mut p = Path::new();
                p.move_to(ax - a * 0.5, ay + a)
                    .line_to(ax + a * 0.7, ay)
                    .line_to(ax - a * 0.5, ay - a)
                    .close();
                c.set_fill(d.accent);
                c.draw_path(p, false, true);
            }
            // 説明
            let lines = caption_lines(&step.caption, pw);
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

/// 下端の余白。デザインの下の飾り(水引・草花など)に本文が重ならないようにする.
fn foot(d: &Design) -> f32 {
    match d.key {
        // ホワイトは右下の線画の花が縦に 24mm ほど伸びる
        "white" => 24.0 * MM,
        _ => (d.bottom_margin - 4.0 * MM).max(MARGIN),
    }
}

fn draw_note(c: &mut Canvas, lines: &[String], d: &Design) {
    c.set_fill(d.text);
    c.set_font(MINCHO, NOTE_SIZE);
    let lh = NOTE_SIZE * 1.6;
    let mut y = foot(d) + lines.len() as f32 * lh;
    for line in lines {
        c.draw_string(MARGIN, y - lh + NOTE_SIZE * 0.42, line);
        y -= lh;
    }
}

/// 地図と道順写真の両方があれば表裏 2 ページ(表: 地図・交通手段・補足、裏: 道順写真)にする.
/// (地図がなくても、1 ページでは道順写真が小さくなりすぎるときは表裏にする)
pub fn is_two_sided(access: &Access) -> bool {
    photos::has(&access.map_photo) && !access.steps.is_empty()
}

fn page(access: &Access, options: &Options, d: &Design, side: &str, draw: impl FnOnce(&mut Canvas)) -> Page {
    let (w, h) = postcard_size(access);
    let mut c = bleed_page(w, h, options.crop_marks);
    draw(&mut c);
    if options.crop_marks {
        draw_crop_marks(&mut c, w, h, BLEED, MARK_AREA - 2.0 * MM);
        c.set_fill(Cmyk::BLACK);
        c.set_font(MINCHO, 5.0);
        let size = if access.is_landscape() {
            "横 148×100mm"
        } else {
            "縦 100×148mm"
        };
        let slug = format!("案内図{side} {} ハガキ {size} 塗り足し3mm CMYK", d.name);
        c.draw_string(-BLEED - MARK_AREA + 1.0 * MM, -BLEED - MARK_AREA + 1.5 * MM, &slug);
    }
    c.finish()
}

fn draw_map(c: &mut Canvas, access: &Access, d: &Design, x: f32, y: f32, w: f32, h: f32) {
    if let Some([ix, iy, iw, ih]) = draw_contain(c, &access.map_photo, x, y, w, h) {
        c.set_stroke(d.sub);
        c.set_line_width(0.4);
        c.rect(ix, iy, iw, ih, true, false);
    }
}

/// 案内図のページ(1 枚、または表裏 2 枚).
pub fn access_pages(event: &Event, access: &Access, design: &str, options: &Options) -> Vec<Page> {
    // 案内図のデザインが選ばれていなければ招待状と同じ
    let d = designs::get(if access.design.is_empty() {
        design
    } else {
        &access.design
    });
    let (w, _) = postcard_size(access);
    let landscape = access.is_landscape();
    let max_cols = if landscape { 4 } else { 3 };
    let content_w = w - 2.0 * MARGIN;
    let has_map = photos::has(&access.map_photo);
    let note_lines = if access.note.is_empty() {
        Vec::new()
    } else {
        wrap(&access.note, MINCHO, NOTE_SIZE, content_w)
    };
    let note_h = note_lines.len() as f32 * NOTE_SIZE * 1.6;
    let bottom = foot(d) + if note_h > 0.0 { note_h + 2.0 * MM } else { 0.0 };
    // 縦向きで交通手段と道順写真を 1 ページに積むと写真が小さくなる(説明が省略される)なら、
    // 写真は裏に回す
    let crowded = !landscape && !access.routes.is_empty() && !access.steps.is_empty() && {
        let (_, h) = postcard_size(access);
        let mut scratch = bleed_page(w, h, false);
        let y = draw_header(&mut scratch, event, access, d);
        let route_limit = (y - bottom - 28.0 * MM - 2.0 * SECTION_GAP).max(12.0 * MM);
        let (_, _, routes_h) = fit_routes(access, content_w, route_limit);
        let rest = (y - bottom - routes_h - SECTION_GAP).max(0.0);
        let (grid, s) = choose_grid(access, content_w, rest, max_cols);
        let pw = step_box(content_w, &grid, s).0;
        pw < MIN_FRONT_PHOTO
            || access
                .steps
                .iter()
                .any(|st| wrap_phrases(&st.caption, MINCHO, CAPTION_SIZE, pw).len() > CAPTION_LINES)
    };
    let two_sided = is_two_sided(access) || crowded;
    // 横向きで交通手段と、地図または道順写真があるときは左右に並べる
    //   地図あり: 左に地図、右に交通手段 / 地図なし: 左に交通手段、右に道順写真
    let steps_on_front = !two_sided && !access.steps.is_empty();
    let side_by_side = landscape && !access.routes.is_empty() && (has_map || steps_on_front);
    const COL_GAP: f32 = 5.0 * MM;
    let (left_w, route_x, route_w) = match (side_by_side, has_map) {
        (true, true) => {
            let map_w = content_w * 0.56;
            (map_w, MARGIN + map_w + COL_GAP, content_w - map_w - COL_GAP)
        }
        (true, false) => {
            let route_w = content_w * 0.42;
            (route_w, MARGIN, route_w)
        }
        _ => (content_w, MARGIN, content_w),
    };
    let front = page(access, options, d, if two_sided { "(表)" } else { "" }, |c| {
        let mut y = draw_header(c, event, access, d);
        let with_steps = steps_on_front;
        // 交通手段に使える高さ: 横に並べるなら列の高さ、縦に積むなら地図と写真の分を残す
        let route_limit = if side_by_side {
            y - bottom
        } else {
            let reserved = if has_map { 30.0 * MM } else { 0.0 } + if with_steps { 28.0 * MM } else { 0.0 };
            (y - bottom - reserved - 2.0 * SECTION_GAP).max(12.0 * MM)
        };
        let (routes, route_style, routes_h) = fit_routes(access, route_w, route_limit);
        if side_by_side {
            let col_h = (y - bottom).clamp(0.0, MAX_MAP);
            if has_map {
                draw_map(c, access, d, MARGIN, y - col_h, left_w, col_h);
                // 交通手段は地図の高さの中で上下の中央に置く
                let offset = ((col_h - routes_h) / 2.0).max(0.0);
                draw_routes(c, &routes, route_style, d, route_x, y - offset);
            } else {
                let steps_x = MARGIN + left_w + COL_GAP;
                let steps_w = content_w - left_w - COL_GAP;
                let (grid, s) = choose_grid(access, steps_w, col_h, 2);
                let steps_h = steps_height(access, steps_w, &grid, s);
                let offset = ((steps_h - routes_h) / 2.0).max(0.0);
                draw_routes(c, &routes, route_style, d, route_x, y - offset);
                draw_steps(c, access, d, steps_x, steps_w, y, &grid, s);
            }
            draw_note(c, &note_lines, d);
            return;
        }
        let sections = [has_map, !routes.is_empty(), with_steps].iter().filter(|b| **b).count() as f32;
        let rest = (y - bottom - routes_h - SECTION_GAP * (sections - 1.0).max(0.0)).max(0.0);
        let (grid, s) = choose_grid(access, content_w, rest, max_cols);
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
            draw_map(c, access, d, MARGIN, y - map_h, content_w, map_h);
            y -= map_h + SECTION_GAP;
        }
        if !routes.is_empty() {
            if !has_map && !with_steps {
                // 交通手段だけの面は上下の中央に置く
                y -= ((y - bottom - routes_h) / 2.0).max(0.0);
            }
            y = draw_routes(c, &routes, route_style, d, MARGIN, y) - SECTION_GAP;
        }
        if with_steps {
            draw_steps(c, access, d, MARGIN, content_w, y, &grid, s);
        }
        draw_note(c, &note_lines, d);
    });
    let mut pages = vec![front];
    if two_sided {
        pages.push(page(access, options, d, "(裏)", |c| {
            draw_steps_page(c, event, access, d, content_w, max_cols)
        }));
    }
    // もう 1 つの道順(お車など)は別のページに
    if !access.car_steps.is_empty() {
        let car = access.car_route();
        pages.push(page(access, options, d, "(お車)", |c| {
            draw_steps_page(c, event, &car, d, content_w, max_cols)
        }));
    }
    pages
}

/// 見出しと道順写真だけのページ.
fn draw_steps_page(c: &mut Canvas, event: &Event, access: &Access, d: &Design, content_w: f32, max_cols: usize) {
    let y = draw_header(c, event, access, d);
    let (grid, s) = choose_grid(access, content_w, y - foot(d), max_cols);
    // 余白が残るときは上下の中央に寄せる
    let spare = (y - foot(d) - steps_height(access, content_w, &grid, s)).max(0.0);
    draw_steps(
        c,
        access,
        d,
        MARGIN,
        content_w,
        y - (spare / 2.0).min(12.0 * MM),
        &grid,
        s,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AccessRoute;

    fn long_routes() -> Access {
        let text = "JR 東京駅 丸の内中央口を出て駅前広場を左へ進み横断歩道を渡り徒歩約五分";
        Access {
            orientation: crate::model::LANDSCAPE.into(),
            routes: ACCESS_MODES
                .iter()
                .map(|(k, _)| AccessRoute {
                    mode: k.to_string(),
                    text: text.into(),
                })
                .collect(),
            ..Access::default()
        }
    }

    #[test]
    fn long_routes_fit_in_column() {
        crate::fonts::init_from_repo();
        let access = long_routes();
        for (w, limit) in [(50.0 * MM, 50.0 * MM), (84.0 * MM, 40.0 * MM), (30.0 * MM, 20.0 * MM)] {
            let (rows, st, h) = fit_routes(&access, w, limit);
            assert_eq!(rows.len(), 8);
            assert!(st.k >= 0.6 - 1e-3);
            // 1 行ずつに減らしても収まらないほど狭い場合だけ超えてよい
            if st.max_lines > 1 {
                assert!(h <= limit + 0.01, "{h} > {limit}");
            }
        }
        let (_, st, _) = fit_routes(&access, 120.0 * MM, 200.0 * MM);
        assert_eq!(st.k, 1.0, "余裕があれば縮めない");
    }
}
