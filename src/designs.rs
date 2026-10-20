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

// ------------------------------------------------------------------ 一覧

pub const DEFAULT_DESIGN: &str = "classic";

pub static DESIGNS: [Design; 4] = [
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
            for col in
                d.colors()
                    .into_iter()
                    .chain([LEAF, LEAF_LIGHT, STEM, BERRY, SAKURA, SAKURA_CENTER, NAVY, MODERN_GOLD])
            {
                assert!(col.total_ink() <= 300.0, "{} {:?}", d.key, col);
            }
        }
    }
}
