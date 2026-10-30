//! 招待状のデザイン定義.
//!
//! 色はすべて CMYK で定義する。透明効果は使わない。
//! 細い線や小さい文字はできるだけ版数を少なくし、総インキ量は 300% 以下に抑える。

use serde::Serialize;

use crate::canvas::{Canvas, Cmyk, Path, MM};

pub type CardFn = fn(&mut Canvas, &Design, f32, f32, f32);
pub type EnvelopeFn = fn(&mut Canvas, &Design, f32, f32);

pub struct Design {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// None は紙白(インキなし)
    pub background: Option<Cmyk>,
    pub accent: Cmyk,
    pub sub: Cmyk,
    pub text: Cmyk,
    pub title: &'static str,
    pub draw_card: CardFn,
    pub draw_envelope: EnvelopeFn,
    /// タイトルを帯の上に紙白の抜き文字で置く
    pub title_on_band: bool,
    /// 本文の下端(装飾と重ならない位置)
    pub bottom_margin: f32,
}

#[derive(Serialize)]
pub struct DesignInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// UI 表示用の近似色
    pub swatches: Vec<String>,
}

impl Design {
    pub fn info(&self) -> DesignInfo {
        let bg = self.background.unwrap_or(Cmyk::PAPER);
        DesignInfo {
            key: self.key,
            name: self.name,
            description: self.description,
            swatches: [bg, self.accent, self.sub].iter().map(Cmyk::approx_hex).collect(),
        }
    }

    pub fn colors(&self) -> Vec<Cmyk> {
        let mut v = vec![self.accent, self.sub, self.text];
        v.extend(self.background);
        v
    }
}

// ------------------------------------------------------------------ 共通パーツ

pub fn fill_background(c: &mut Canvas, d: &Design, w: f32, h: f32, bleed: f32) {
    if let Some(bg) = d.background {
        c.set_fill(bg);
        c.rect(-bleed, -bleed, w + 2.0 * bleed, h + 2.0 * bleed, false, true);
    }
}

/// 中央に菱形を置いた飾り罫.
pub fn divider(c: &mut Canvas, cx: f32, y: f32, width: f32, color: Cmyk) {
    c.set_stroke(color);
    c.set_fill(color);
    c.set_line_width(0.4);
    let gap = 2.2 * MM;
    c.line(cx - width / 2.0, y, cx - gap, y);
    c.line(cx + gap, y, cx + width / 2.0, y);
    let d = 1.1 * MM;
    let mut p = Path::new();
    p.move_to(cx, y + d)
        .line_to(cx + d, y)
        .line_to(cx, y - d)
        .line_to(cx - d, y)
        .close();
    c.draw_path(p, false, true);
}

/// 額縁の角飾り。(x, y) が角、sx/sy は内側への向き(±1).
fn corner_flourish(c: &mut Canvas, x: f32, y: f32, sx: f32, sy: f32, s: f32) {
    c.save();
    c.translate(x, y);
    c.scale(sx, sy);
    let mut p = Path::new();
    p.move_to(0.0, 7.0 * s)
        .curve_to(0.0, 2.0 * s, 2.0 * s, 0.0, 7.0 * s, 0.0);
    c.draw_path(p, true, false);
    // 渦巻き
    let mut p = Path::new();
    p.move_to(2.2 * s, 2.2 * s)
        .curve_to(5.0 * s, 5.0 * s, 9.0 * s, 3.5 * s, 8.5 * s, 1.6 * s)
        .curve_to(8.2 * s, 0.6 * s, 6.6 * s, 0.8 * s, 6.9 * s, 2.0 * s);
    c.draw_path(p, true, false);
    let mut p = Path::new();
    p.move_to(2.2 * s, 2.2 * s)
        .curve_to(5.0 * s, 5.0 * s, 3.5 * s, 9.0 * s, 1.6 * s, 8.5 * s)
        .curve_to(0.6 * s, 8.2 * s, 0.8 * s, 6.6 * s, 2.0 * s, 6.9 * s);
    c.draw_path(p, true, false);
    c.circle(2.2 * s, 2.2 * s, 0.55 * s, false, true);
    c.restore();
}

/// 原点から +x 方向に伸びる葉.
fn leaf(c: &mut Canvas, length: f32, width: f32) {
    let mut p = Path::new();
    p.move_to(0.0, 0.0)
        .curve_to(length * 0.3, width, length * 0.75, width * 0.9, length, 0.0)
        .curve_to(length * 0.75, -width * 0.9, length * 0.3, -width, 0.0, 0.0)
        .close();
    c.draw_path(p, false, true);
}

/// 茎に葉が交互につく枝.
#[allow(clippy::too_many_arguments)]
fn sprig(
    c: &mut Canvas,
    x: f32,
    y: f32,
    angle: f32,
    length: f32,
    leaf_color: Cmyk,
    stem: Cmyk,
    n: usize,
    leaf_len: f32,
) {
    c.save();
    c.translate(x, y);
    c.rotate(angle);
    c.set_stroke(stem);
    c.set_line_width(0.6);
    let mut p = Path::new();
    p.move_to(0.0, 0.0)
        .curve_to(length * 0.35, length * 0.06, length * 0.7, length * 0.05, length, 0.0);
    c.draw_path(p, true, false);
    c.set_fill(leaf_color);
    for i in 0..n {
        let t = (i + 1) as f32 / (n + 1) as f32;
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        c.save();
        c.translate(length * t, length * 0.05 * (std::f32::consts::PI * t).sin());
        c.rotate(side * 48.0);
        leaf(c, leaf_len * (1.05 - 0.4 * t), leaf_len * 0.28);
        c.restore();
    }
    c.save();
    c.translate(length, 0.0);
    leaf(c, leaf_len * 0.7, leaf_len * 0.22);
    c.restore();
    c.restore();
}

/// 桜の花(切れ込みのある 5 枚の花びら).
fn sakura(c: &mut Canvas, x: f32, y: f32, r: f32, petal: Cmyk, center: Cmyk, rotation: f32) {
    c.save();
    c.translate(x, y);
    c.rotate(rotation);
    c.set_fill(petal);
    for i in 0..5 {
        c.save();
        c.rotate(i as f32 * 72.0);
        let mut p = Path::new();
        p.move_to(0.0, 0.0)
            .curve_to(-r * 0.55, r * 0.35, -r * 0.6, r * 0.85, -r * 0.22, r)
            .line_to(0.0, r * 0.84)
            .line_to(r * 0.22, r)
            .curve_to(r * 0.6, r * 0.85, r * 0.55, r * 0.35, 0.0, 0.0)
            .close();
        c.draw_path(p, false, true);
        c.restore();
    }
    c.set_fill(center);
    c.set_stroke(center);
    c.set_line_width((r * 0.03).max(0.25));
    for i in 0..5 {
        let a = (i as f32 * 72.0 + 36.0 + 90.0).to_radians();
        c.line(0.0, 0.0, r * 0.38 * a.cos(), r * 0.38 * a.sin());
        c.circle(r * 0.42 * a.cos(), r * 0.42 * a.sin(), r * 0.05, false, true);
    }
    c.circle(0.0, 0.0, r * 0.1, false, true);
    c.restore();
}

fn petal(c: &mut Canvas, x: f32, y: f32, r: f32, color: Cmyk, rotation: f32) {
    c.save();
    c.translate(x, y);
    c.rotate(rotation);
    c.set_fill(color);
    let mut p = Path::new();
    p.move_to(0.0, 0.0)
        .curve_to(-r * 0.6, r * 0.4, -r * 0.5, r, -r * 0.18, r * 1.1)
        .line_to(0.0, r * 0.95)
        .line_to(r * 0.18, r * 1.1)
        .curve_to(r * 0.5, r, r * 0.6, r * 0.4, 0.0, 0.0)
        .close();
    c.draw_path(p, false, true);
    c.restore();
}

// ------------------------------------------------------------------ クラシック

const CLASSIC_GOLD: Cmyk = Cmyk(0.10, 0.30, 0.75, 0.10);

fn classic_card(c: &mut Canvas, d: &Design, w: f32, h: f32, bleed: f32) {
    fill_background(c, d, w, h, bleed);
    let inset = 5.0 * MM;
    c.set_stroke(d.accent);
    c.set_fill(d.accent);
    c.set_line_width(0.9);
    c.rect(inset, inset, w - 2.0 * inset, h - 2.0 * inset, true, false);
    c.set_line_width(0.3);
    let i2 = inset + 1.3 * MM;
    c.rect(i2, i2, w - 2.0 * i2, h - 2.0 * i2, true, false);
    c.set_line_width(0.45);
    let i3 = i2 + 1.0 * MM;
    for (x, y, sx, sy) in [
        (i3, i3, 1.0, 1.0),
        (w - i3, i3, -1.0, 1.0),
        (i3, h - i3, 1.0, -1.0),
        (w - i3, h - i3, -1.0, -1.0),
    ] {
        corner_flourish(c, x, y, sx, sy, 0.9 * MM);
    }
}

fn classic_envelope(c: &mut Canvas, d: &Design, w: f32, h: f32) {
    c.set_stroke(d.accent);
    c.set_line_width(0.5);
    c.rect(7.0 * MM, 7.0 * MM, w - 14.0 * MM, h - 14.0 * MM, true, false);
    c.set_line_width(0.4);
    c.set_fill(d.accent);
    corner_flourish(c, 8.0 * MM, 8.0 * MM, 1.0, 1.0, 0.8 * MM);
    corner_flourish(c, w - 8.0 * MM, h - 8.0 * MM, -1.0, -1.0, 0.8 * MM);
}

// ------------------------------------------------------------------ ボタニカル

const LEAF: Cmyk = Cmyk(0.55, 0.15, 0.60, 0.15);
const LEAF_LIGHT: Cmyk = Cmyk(0.35, 0.05, 0.40, 0.05);
const STEM: Cmyk = Cmyk(0.50, 0.20, 0.55, 0.35);
const BERRY: Cmyk = Cmyk(0.05, 0.35, 0.25, 0.05);

fn botanical_card(c: &mut Canvas, d: &Design, w: f32, h: f32, bleed: f32) {
    fill_background(c, d, w, h, bleed);
    // 左上と右下の角に、辺に沿わせて枝を配置(本文にかからないように)
    let ll = 8.0 * MM;
    sprig(c, -3.0 * MM, h - 4.0 * MM, -8.0, 34.0 * MM, LEAF, STEM, 6, ll);
    sprig(c, 3.0 * MM, h + 3.0 * MM, -80.0, 40.0 * MM, LEAF_LIGHT, STEM, 6, ll);
    sprig(c, w + 3.0 * MM, 4.0 * MM, 172.0, 34.0 * MM, LEAF, STEM, 6, ll);
    sprig(c, w - 3.0 * MM, -3.0 * MM, 100.0, 40.0 * MM, LEAF_LIGHT, STEM, 6, ll);
    c.set_fill(BERRY);
    for (x, y, r) in [
        (9.0, h / MM - 9.0, 1.3),
        (12.5, h / MM - 7.0, 1.0),
        (w / MM - 9.0, 9.0, 1.3),
        (w / MM - 12.5, 7.0, 1.0),
    ] {
        c.circle(x * MM, y * MM, r * MM, false, true);
    }
    c.set_stroke(d.sub);
    c.set_line_width(0.35);
    c.rect(6.0 * MM, 6.0 * MM, w - 12.0 * MM, h - 12.0 * MM, true, false);
}

fn botanical_envelope(c: &mut Canvas, _d: &Design, _w: f32, _h: f32) {
    sprig(c, 6.0 * MM, 8.0 * MM, 35.0, 30.0 * MM, LEAF, STEM, 6, 9.6 * MM);
    sprig(c, 8.0 * MM, 7.0 * MM, 12.0, 22.0 * MM, LEAF_LIGHT, STEM, 5, 7.0 * MM);
    c.set_fill(BERRY);
    c.circle(10.0 * MM, 15.0 * MM, 1.1 * MM, false, true);
}

// ------------------------------------------------------------------ 桜

const SAKURA: Cmyk = Cmyk(0.0, 0.32, 0.10, 0.0);
const SAKURA_LIGHT: Cmyk = Cmyk(0.0, 0.16, 0.05, 0.0);
const SAKURA_CENTER: Cmyk = Cmyk(0.0, 0.70, 0.30, 0.10);

fn sakura_card(c: &mut Canvas, d: &Design, w: f32, h: f32, bleed: f32) {
    fill_background(c, d, w, h, bleed);
    let (wm, hm) = (w / MM, h / MM);
    let flowers = [
        (wm - 6.0, hm - 7.0, 9.0, SAKURA, 10.0),
        (wm - 20.0, hm - 4.0, 6.0, SAKURA_LIGHT, 40.0),
        (wm - 4.0, hm - 21.0, 5.5, SAKURA_LIGHT, 25.0),
        (wm - 16.0, hm - 14.0, 3.2, SAKURA, 60.0),
        (6.0, 6.0, 8.0, SAKURA, 30.0),
        (19.0, 3.0, 5.0, SAKURA_LIGHT, 5.0),
        (3.0, 19.0, 4.5, SAKURA_LIGHT, 50.0),
    ];
    for (x, y, r, col, rot) in flowers {
        sakura(c, x * MM, y * MM, r * MM, col, SAKURA_CENTER, rot);
    }
    for (x, y, r, rot) in [
        (wm - 30.0, hm - 13.0, 1.6, 30.0),
        (wm - 9.0, hm - 31.0, 1.4, 120.0),
        (30.0, 7.0, 1.4, 200.0),
        (8.0, 28.0, 1.2, 280.0),
        (wm - 26.0, hm - 25.0, 1.1, 75.0),
    ] {
        petal(c, x * MM, y * MM, r * MM, SAKURA, rot);
    }
    c.set_stroke(d.accent);
    c.set_line_width(0.35);
    c.rect(5.0 * MM, 5.0 * MM, w - 10.0 * MM, h - 10.0 * MM, true, false);
}

fn sakura_envelope(c: &mut Canvas, _d: &Design, _w: f32, _h: f32) {
    sakura(c, 12.0 * MM, 13.0 * MM, 6.0 * MM, SAKURA, SAKURA_CENTER, 20.0);
    sakura(c, 21.0 * MM, 9.0 * MM, 3.8 * MM, SAKURA_LIGHT, SAKURA_CENTER, 50.0);
    petal(c, 24.0 * MM, 17.0 * MM, 1.4 * MM, SAKURA, 210.0);
}

// ------------------------------------------------------------------ モダン

const NAVY: Cmyk = Cmyk(1.0, 0.80, 0.30, 0.30);
const MODERN_GOLD: Cmyk = Cmyk(0.15, 0.30, 0.65, 0.05);
pub const MODERN_BAND: f32 = 24.0 * MM;

fn modern_card(c: &mut Canvas, _d: &Design, w: f32, h: f32, bleed: f32) {
    c.set_fill(NAVY);
    c.rect(
        -bleed,
        h - MODERN_BAND,
        w + 2.0 * bleed,
        MODERN_BAND + bleed,
        false,
        true,
    );
    c.rect(-bleed, -bleed, w + 2.0 * bleed, 6.0 * MM + bleed, false, true);
    c.set_stroke(MODERN_GOLD);
    c.set_line_width(0.6);
    c.line(
        -bleed,
        h - MODERN_BAND - 1.2 * MM,
        w + bleed,
        h - MODERN_BAND - 1.2 * MM,
    );
    c.line(-bleed, 7.2 * MM, w + bleed, 7.2 * MM);
}

fn modern_envelope(c: &mut Canvas, _d: &Design, _w: f32, _h: f32) {
    c.set_fill(NAVY);
    c.rect(8.0 * MM, 8.0 * MM, 1.2 * MM, 26.0 * MM, false, true);
    c.set_fill(MODERN_GOLD);
    c.rect(10.2 * MM, 8.0 * MM, 0.4 * MM, 26.0 * MM, false, true);
}

// ------------------------------------------------------------------ 睦月(一月)

const BENI: Cmyk = Cmyk(0.0, 0.90, 0.70, 0.10);
const BENI_LIGHT: Cmyk = Cmyk(0.0, 0.45, 0.25, 0.0);
const PINE: Cmyk = Cmyk(0.75, 0.20, 0.75, 0.30);
const BAMBOO: Cmyk = Cmyk(0.55, 0.10, 0.60, 0.15);
const BRANCH: Cmyk = Cmyk(0.30, 0.55, 0.70, 0.45);

/// 梅の花(丸い 5 枚の花びら)。petal が None なら白梅(紙白に金の輪郭).
fn plum(c: &mut Canvas, x: f32, y: f32, r: f32, petal: Option<Cmyk>, rotation: f32) {
    c.save();
    c.translate(x, y);
    c.rotate(rotation);
    c.set_fill(petal.unwrap_or(Cmyk::PAPER));
    c.set_stroke(CLASSIC_GOLD);
    c.set_line_width(0.3);
    for i in 0..5 {
        let a = (i as f32 * 72.0 + 90.0).to_radians();
        c.circle(r * 0.5 * a.cos(), r * 0.5 * a.sin(), r * 0.5, petal.is_none(), true);
    }
    // しべ
    c.set_stroke(CLASSIC_GOLD);
    c.set_fill(CLASSIC_GOLD);
    c.set_line_width((r * 0.04).max(0.25));
    for i in 0..8 {
        let a = (i as f32 * 45.0 + 22.5).to_radians();
        c.line(0.0, 0.0, r * 0.42 * a.cos(), r * 0.42 * a.sin());
        c.circle(r * 0.45 * a.cos(), r * 0.45 * a.sin(), r * 0.05, false, true);
    }
    c.set_fill(if petal.is_some() { CLASSIC_GOLD } else { BENI });
    c.circle(0.0, 0.0, r * 0.12, false, true);
    c.restore();
}

fn plum_bud(c: &mut Canvas, x: f32, y: f32, r: f32, color: Cmyk) {
    c.set_fill(color);
    c.circle(x, y, r, false, true);
}

/// 松葉を扇状に広げた「松」. angle は扇の中心の向き.
fn pine(c: &mut Canvas, x: f32, y: f32, r: f32, angle: f32) {
    c.save();
    c.translate(x, y);
    c.set_stroke(PINE);
    c.set_line_width(0.5);
    for i in 0..13 {
        let a = (angle - 72.0 + i as f32 * 12.0).to_radians();
        c.line(0.0, 0.0, r * a.cos(), r * a.sin());
    }
    c.set_fill(BRANCH);
    c.circle(0.0, 0.0, r * 0.09, false, true);
    c.restore();
}

/// 紅白・金の三本の水引を波打たせて横に渡す.
fn mizuhiki(c: &mut Canvas, y: f32, x0: f32, x1: f32) {
    let period = 24.0 * MM;
    let amp = 1.4 * MM;
    for (offset, color) in [(-1.6, BENI), (0.0, CLASSIC_GOLD), (1.6, BENI)] {
        c.set_stroke(color);
        c.set_line_width(1.1);
        let mut p = Path::new();
        let mut x = x0;
        let base = y + offset;
        p.move_to(x, base);
        while x < x1 {
            let half = period / 2.0;
            p.curve_to(x + half * 0.36, base + amp, x + half * 0.64, base + amp, x + half, base);
            p.curve_to(
                x + half * 1.36,
                base - amp,
                x + half * 1.64,
                base - amp,
                x + period,
                base,
            );
            x += period;
        }
        c.draw_path(p, true, false);
    }
}

fn mutsuki_card(c: &mut Canvas, d: &Design, w: f32, h: f32, bleed: f32) {
    fill_background(c, d, w, h, bleed);
    c.set_stroke(CLASSIC_GOLD);
    c.set_line_width(0.4);
    c.rect(5.0 * MM, 5.0 * MM, w - 10.0 * MM, h - 10.0 * MM, true, false);
    mizuhiki(c, 9.0 * MM, -bleed, w + bleed);

    // 左上: 紅白の梅の枝
    let top = h / MM;
    c.set_stroke(BRANCH);
    c.set_line_width(1.3);
    let mut p = Path::new();
    p.move_to(-3.0 * MM, (top - 27.0) * MM).curve_to(
        5.0 * MM,
        (top - 20.0) * MM,
        11.0 * MM,
        (top - 10.0) * MM,
        27.0 * MM,
        (top - 3.0) * MM,
    );
    c.draw_path(p, true, false);
    c.set_line_width(0.8);
    let mut p = Path::new();
    p.move_to(6.0 * MM, (top - 17.0) * MM).curve_to(
        3.5 * MM,
        (top - 12.0) * MM,
        3.0 * MM,
        (top - 9.0) * MM,
        3.5 * MM,
        (top - 5.0) * MM,
    );
    c.draw_path(p, true, false);
    plum(c, 4.0 * MM, (top - 21.0) * MM, 3.4 * MM, Some(BENI), 10.0);
    plum(c, 10.0 * MM, (top - 13.0) * MM, 2.8 * MM, None, 40.0);
    plum(c, 18.0 * MM, (top - 7.5) * MM, 3.0 * MM, Some(BENI), 70.0);
    plum(c, 3.5 * MM, (top - 7.0) * MM, 2.2 * MM, None, 20.0);
    plum_bud(c, 25.0 * MM, (top - 4.0) * MM, 1.0 * MM, BENI_LIGHT);
    plum_bud(c, 14.0 * MM, (top - 11.5) * MM, 0.8 * MM, BENI);
    plum_bud(c, 3.6 * MM, (top - 4.0) * MM, 0.8 * MM, BENI);

    // 右下: 松と笹
    let right = w / MM;
    c.set_fill(BAMBOO);
    for angle in [118.0, 142.0, 165.0] {
        c.save();
        c.translate((right - 22.0) * MM, 7.0 * MM);
        c.rotate(angle);
        leaf(c, 10.0 * MM, 1.6 * MM);
        c.restore();
    }
    pine(c, (right - 12.0) * MM, 10.0 * MM, 6.5 * MM, 95.0);
    pine(c, (right - 5.0) * MM, 15.0 * MM, 4.6 * MM, 110.0);
    pine(c, (right - 19.0) * MM, 5.0 * MM, 5.0 * MM, 80.0);

    // 左下: 水引に添える梅
    plum(c, 10.0 * MM, 10.5 * MM, 3.0 * MM, Some(BENI), 0.0);
    plum(c, 16.0 * MM, 7.0 * MM, 2.3 * MM, None, 30.0);
}

fn mutsuki_envelope(c: &mut Canvas, _d: &Design, _w: f32, _h: f32) {
    mizuhiki(c, 10.0 * MM, 6.0 * MM, 40.0 * MM);
    plum(c, 10.0 * MM, 13.0 * MM, 3.0 * MM, Some(BENI), 0.0);
    plum(c, 16.0 * MM, 10.0 * MM, 2.3 * MM, None, 30.0);
    plum_bud(c, 20.0 * MM, 14.0 * MM, 0.8 * MM, BENI);
}

// ------------------------------------------------------------------ 雪

const SILVER_GROUND: Cmyk = Cmyk(0.08, 0.04, 0.02, 0.03);
const SILVER: Cmyk = Cmyk(0.25, 0.15, 0.08, 0.20);
const SILVER_LIGHT: Cmyk = Cmyk(0.14, 0.08, 0.04, 0.08);
const INDIGO: Cmyk = Cmyk(0.90, 0.65, 0.20, 0.30);

/// 雪輪文様(ゆるくふくらむ 6 つの弧と、その間の小さな丸い切れ込み)の輪郭.
fn yukiwa_path(r: f32) -> Path {
    let pt = |deg: f32, rad: f32| {
        let a = deg.to_radians();
        (rad * a.cos(), rad * a.sin())
    };
    let mut p = Path::new();
    let (x, y) = pt(94.0, r * 0.95);
    p.move_to(x, y);
    for i in 0..6 {
        let a = 90.0 + i as f32 * 60.0;
        // ふくらみ
        let (x1, y1) = pt(a + 20.0, r * 1.07);
        let (x2, y2) = pt(a + 40.0, r * 1.07);
        let (x, y) = pt(a + 56.0, r * 0.95);
        p.curve_to(x1, y1, x2, y2, x, y);
        // 小さな丸い切れ込み
        let (cx1, cy1) = pt(a + 56.5, r * 0.80);
        let (cx2, cy2) = pt(a + 63.5, r * 0.80);
        let (x, y) = pt(a + 64.0, r * 0.95);
        p.curve_to(cx1, cy1, cx2, cy2, x, y);
    }
    p.close();
    p
}

/// 雪輪を 1 つ描く。fill があれば塗り、line があればその色で輪郭を描く.
fn yukiwa(c: &mut Canvas, x: f32, y: f32, r: f32, fill: Option<Cmyk>, line: Option<Cmyk>, rotation: f32) {
    c.save();
    c.translate(x, y);
    c.rotate(rotation);
    if let Some(col) = fill {
        c.set_fill(col);
    }
    if let Some(col) = line {
        c.set_stroke(col);
        c.set_line_width((r * 0.035).clamp(0.3, 0.8));
    }
    c.draw_path(yukiwa_path(r), line.is_some(), fill.is_some());
    c.restore();
}

/// 紙白の雪輪の内側に、銀の細い雪輪を重ねる.
fn yukiwa_double(c: &mut Canvas, x: f32, y: f32, r: f32, rotation: f32) {
    yukiwa(c, x, y, r, Some(Cmyk::PAPER), None, rotation);
    yukiwa(c, x, y, r * 0.72, None, Some(SILVER), rotation);
}

/// 決まった並びの擬似乱数(毎回同じ位置に雪を降らせる).
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// 本文の領域を避けて、舞う雪(丸い粒)を散らす.
fn falling_snow(c: &mut Canvas, w: f32, h: f32, avoid: [f32; 4], color: Cmyk, count: usize, seed: u32) {
    let mut rng = Lcg(seed);
    c.set_fill(color);
    let mut placed = 0;
    while placed < count {
        let (x, y) = (rng.next() * w, rng.next() * h);
        let r = (0.35 + rng.next() * 0.75) * MM;
        if x + r > avoid[0] && x - r < avoid[2] && y + r > avoid[1] && y - r < avoid[3] {
            continue;
        }
        c.circle(x, y, r, false, true);
        placed += 1;
    }
}

fn yuki_card(c: &mut Canvas, d: &Design, w: f32, h: f32, bleed: f32) {
    fill_background(c, d, w, h, bleed);
    c.set_stroke(SILVER);
    c.set_line_width(0.4);
    c.rect(5.0 * MM, 5.0 * MM, w - 10.0 * MM, h - 10.0 * MM, true, false);

    let text_area = [12.0 * MM, 17.0 * MM, w - 12.0 * MM, h - 12.0 * MM];
    falling_snow(c, w, h, text_area, Cmyk::PAPER, 22, 7);
    falling_snow(c, w, h, text_area, SILVER_LIGHT, 8, 11);

    let (wm, hm) = (w / MM, h / MM);
    // 右上
    yukiwa_double(c, (wm - 3.0) * MM, (hm - 5.0) * MM, 12.0 * MM, 8.0);
    yukiwa(c, (wm - 24.0) * MM, (hm - 4.5) * MM, 4.5 * MM, None, Some(SILVER), 20.0);
    yukiwa(
        c,
        (wm - 5.0) * MM,
        (hm - 25.0) * MM,
        3.2 * MM,
        Some(SILVER_LIGHT),
        None,
        0.0,
    );
    // 左上
    yukiwa(
        c,
        7.0 * MM,
        (hm - 12.0) * MM,
        6.0 * MM,
        Some(Cmyk::PAPER),
        Some(SILVER_LIGHT),
        15.0,
    );
    yukiwa(c, 3.5 * MM, (hm - 28.0) * MM, 3.0 * MM, None, Some(SILVER), 0.0);
    // 左下
    yukiwa_double(c, 4.0 * MM, 7.0 * MM, 12.0 * MM, 25.0);
    yukiwa(c, 25.0 * MM, 4.5 * MM, 4.5 * MM, None, Some(SILVER), 10.0);
    yukiwa(c, 4.0 * MM, 32.0 * MM, 3.0 * MM, Some(Cmyk::PAPER), None, 0.0);
    // 右下
    yukiwa(c, (wm - 8.0) * MM, 11.0 * MM, 6.0 * MM, Some(SILVER_LIGHT), None, 30.0);
    yukiwa(c, (wm - 4.0) * MM, 25.0 * MM, 3.0 * MM, None, Some(SILVER), 0.0);
}

fn yuki_envelope(c: &mut Canvas, _d: &Design, _w: f32, _h: f32) {
    yukiwa(c, 12.0 * MM, 14.0 * MM, 6.0 * MM, None, Some(SILVER), 10.0);
    yukiwa(c, 12.0 * MM, 14.0 * MM, 4.3 * MM, None, Some(SILVER_LIGHT), 10.0);
    yukiwa(c, 21.0 * MM, 9.0 * MM, 3.2 * MM, Some(SILVER_LIGHT), None, 0.0);
    yukiwa(c, 22.0 * MM, 19.0 * MM, 2.0 * MM, None, Some(SILVER), 30.0);
}

// ------------------------------------------------------------------ ホワイト

/// 線画の色(あたたかみのあるグレー)
const LINE_GRAY: Cmyk = Cmyk(0.10, 0.12, 0.18, 0.38);
const LINE_GRAY_LIGHT: Cmyk = Cmyk(0.05, 0.06, 0.10, 0.22);
const WHITE_TITLE: Cmyk = Cmyk(0.15, 0.18, 0.25, 0.55);

/// 線画の葉(原点から +x 方向)。内側は紙の白で塗り、後ろの線を隠す.
fn line_leaf(c: &mut Canvas, length: f32, width: f32) {
    let mut p = Path::new();
    p.move_to(0.0, 0.0)
        .curve_to(length * 0.3, width, length * 0.75, width * 0.9, length, 0.0)
        .curve_to(length * 0.75, -width * 0.9, length * 0.3, -width, 0.0, 0.0)
        .close();
    c.set_fill(Cmyk::PAPER);
    c.draw_path(p, true, true);
    // 葉脈
    let mut p = Path::new();
    p.move_to(length * 0.08, 0.0).curve_to(
        length * 0.4,
        width * 0.08,
        length * 0.7,
        width * 0.05,
        length * 0.92,
        0.0,
    );
    c.draw_path(p, true, false);
}

/// 線画の花(丸みのある 6 枚の花びらを二重に重ねる)。内側は紙の白で塗る.
fn line_flower(c: &mut Canvas, x: f32, y: f32, r: f32, rotation: f32) {
    c.save();
    c.translate(x, y);
    c.rotate(rotation);
    c.set_fill(Cmyk::PAPER);
    for (scale, offset) in [(1.0, 0.0), (0.62, 30.0)] {
        let rr = r * scale;
        for i in 0..6 {
            c.save();
            c.rotate(offset + i as f32 * 60.0);
            let mut p = Path::new();
            p.move_to(0.0, rr * 0.18)
                .curve_to(-rr * 0.42, rr * 0.45, -rr * 0.32, rr * 0.98, 0.0, rr)
                .curve_to(rr * 0.32, rr * 0.98, rr * 0.42, rr * 0.45, 0.0, rr * 0.18)
                .close();
            c.draw_path(p, true, true);
            c.restore();
        }
    }
    c.circle(0.0, 0.0, r * 0.14, true, true);
    c.restore();
}

/// 線画の枝(茎に輪郭だけの葉).
fn line_sprig(c: &mut Canvas, x: f32, y: f32, angle: f32, length: f32, n: usize) {
    c.save();
    c.translate(x, y);
    c.rotate(angle);
    let mut p = Path::new();
    p.move_to(0.0, 0.0)
        .curve_to(length * 0.35, length * 0.05, length * 0.7, length * 0.04, length, 0.0);
    c.draw_path(p, true, false);
    for i in 0..n {
        let t = (i + 1) as f32 / (n + 1) as f32;
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        c.save();
        c.translate(length * t, length * 0.045 * (std::f32::consts::PI * t).sin());
        c.rotate(side * 50.0);
        line_leaf(c, length * 0.26 * (1.05 - 0.35 * t), length * 0.07);
        c.restore();
    }
    c.restore();
}

/// 角の線画の花束(花 2 輪と枝).
fn white_corner(c: &mut Canvas, x: f32, y: f32, sx: f32, sy: f32, s: f32) {
    c.save();
    c.translate(x, y);
    c.scale(sx, sy);
    c.set_stroke(LINE_GRAY_LIGHT);
    c.set_line_width(0.35);
    line_sprig(c, 2.0 * s, 2.0 * s, 8.0, 30.0 * s, 6);
    line_sprig(c, 2.0 * s, 2.0 * s, 72.0, 26.0 * s, 5);
    c.set_stroke(LINE_GRAY);
    c.set_line_width(0.4);
    line_flower(c, 6.5 * s, 6.5 * s, 5.2 * s, 10.0);
    line_flower(c, 15.0 * s, 4.0 * s, 3.2 * s, 35.0);
    line_flower(c, 4.0 * s, 15.0 * s, 2.6 * s, 50.0);
    c.restore();
}

/// 左上と右下の角を gap だけ開けた枠。角の線画の花に線が重ならないようにする.
fn open_frame(c: &mut Canvas, w: f32, h: f32, inset: f32, gap: f32) {
    let (l, r, b, t) = (inset, w - inset, inset, h - inset);
    let mut left_bottom = Path::new();
    left_bottom.move_to(l, t - gap).line_to(l, b).line_to(r - gap, b);
    c.draw_path(left_bottom, true, false);
    let mut top_right = Path::new();
    top_right.move_to(l + gap, t).line_to(r, t).line_to(r, b + gap);
    c.draw_path(top_right, true, false);
}

fn white_card(c: &mut Canvas, _d: &Design, w: f32, h: f32, _bleed: f32) {
    // 背景は紙の白のまま(インキを乗せない)
    c.set_stroke(LINE_GRAY_LIGHT);
    c.set_line_width(0.3);
    open_frame(c, w, h, 6.0 * MM, 33.0 * MM);
    c.set_line_width(0.2);
    open_frame(c, w, h, 7.2 * MM, 33.0 * MM);
    white_corner(c, 4.5 * MM, h - 4.5 * MM, 1.0, -1.0, MM * 0.95);
    white_corner(c, w - 4.5 * MM, 4.5 * MM, -1.0, 1.0, MM * 0.95);
}

fn white_envelope(c: &mut Canvas, _d: &Design, _w: f32, _h: f32) {
    white_corner(c, 7.0 * MM, 7.0 * MM, 1.0, 1.0, MM * 0.75);
}

// ------------------------------------------------------------------ 一覧

pub const DEFAULT_DESIGN: &str = "classic";

pub static DESIGNS: [Design; 7] = [
    Design {
        key: "classic",
        name: "クラシック",
        description: "アイボリーにゴールドの額縁飾り。格式ある王道デザイン",
        background: Some(Cmyk(0.0, 0.03, 0.10, 0.0)),
        accent: CLASSIC_GOLD,
        sub: CLASSIC_GOLD,
        text: Cmyk(0.0, 0.10, 0.20, 0.82),
        title: "Wedding Invitation",
        draw_card: classic_card,
        draw_envelope: classic_envelope,
        title_on_band: false,
        bottom_margin: 13.0 * MM,
    },
    Design {
        key: "botanical",
        name: "ボタニカル",
        description: "ユーカリの枝葉をあしらったナチュラルなデザイン",
        background: Some(Cmyk(0.0, 0.0, 0.03, 0.0)),
        accent: LEAF,
        sub: LEAF_LIGHT,
        text: Cmyk(0.20, 0.0, 0.15, 0.80),
        title: "Wedding Invitation",
        draw_card: botanical_card,
        draw_envelope: botanical_envelope,
        title_on_band: false,
        bottom_margin: 17.0 * MM,
    },
    Design {
        key: "sakura",
        name: "桜",
        description: "淡い桜色に桜の花が舞う和のデザイン",
        background: Some(Cmyk(0.0, 0.05, 0.02, 0.0)),
        accent: SAKURA_CENTER,
        sub: SAKURA,
        text: Cmyk(0.0, 0.15, 0.05, 0.82),
        title: "Wedding Invitation",
        draw_card: sakura_card,
        draw_envelope: sakura_envelope,
        title_on_band: false,
        bottom_margin: 17.0 * MM,
    },
    Design {
        key: "modern",
        name: "モダン",
        description: "ネイビーの帯にゴールドのラインを効かせたシンプルモダン",
        background: None,
        accent: NAVY,
        sub: MODERN_GOLD,
        text: Cmyk(0.30, 0.20, 0.0, 0.80),
        title: "Wedding Invitation",
        draw_card: modern_card,
        draw_envelope: modern_envelope,
        title_on_band: true,
        bottom_margin: 15.0 * MM,
    },
    Design {
        key: "mutsuki",
        name: "睦月(一月)",
        description: "紅白の梅と松、金の水引をあしらったお正月らしい和のデザイン",
        background: Some(Cmyk(0.0, 0.04, 0.12, 0.0)),
        accent: BENI,
        sub: CLASSIC_GOLD,
        text: Cmyk(0.0, 0.20, 0.30, 0.85),
        title: "Wedding Invitation",
        draw_card: mutsuki_card,
        draw_envelope: mutsuki_envelope,
        title_on_band: false,
        bottom_margin: 17.0 * MM,
    },
    Design {
        key: "yuki",
        name: "雪",
        description: "銀鼠の地に雪輪文様を散らした、しっとり上品な和の冬デザイン",
        background: Some(SILVER_GROUND),
        accent: INDIGO,
        sub: SILVER,
        text: Cmyk(0.30, 0.15, 0.0, 0.80),
        title: "Wedding Invitation",
        draw_card: yuki_card,
        draw_envelope: yuki_envelope,
        title_on_band: false,
        bottom_margin: 18.0 * MM,
    },
    Design {
        key: "white",
        name: "ホワイト",
        description: "紙の白を活かし、グレーの線画の花と細い二重の額縁で仕上げた清楚なデザイン",
        background: None,
        accent: WHITE_TITLE,
        sub: LINE_GRAY_LIGHT,
        text: Cmyk(0.0, 0.0, 0.0, 0.82),
        title: "Wedding Invitation",
        draw_card: white_card,
        draw_envelope: white_envelope,
        title_on_band: false,
        bottom_margin: 16.0 * MM,
    },
];

pub fn find(key: &str) -> Option<&'static Design> {
    DESIGNS.iter().find(|d| d.key == key)
}

pub fn get(key: &str) -> &'static Design {
    find(key).unwrap_or(&DESIGNS[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_ink_is_within_limit() {
        for d in &DESIGNS {
            for col in d.colors().into_iter().chain([
                LINE_GRAY,
                LINE_GRAY_LIGHT,
                WHITE_TITLE,
                LEAF,
                LEAF_LIGHT,
                STEM,
                BERRY,
                SAKURA,
                SAKURA_CENTER,
                NAVY,
                MODERN_GOLD,
                BENI,
                BENI_LIGHT,
                PINE,
                BAMBOO,
                BRANCH,
            ]) {
                assert!(col.total_ink() <= 300.0, "{} {:?}", d.key, col);
            }
        }
    }
}
