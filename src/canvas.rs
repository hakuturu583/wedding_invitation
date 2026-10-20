//! 描画命令の記録.
//!
//! レイアウトは [`Canvas`] に描画命令(ディスプレイリスト)として記録し、
//! 同じ命令列を PDF(入稿用)と SVG(画面プレビュー用)の両方に書き出す。
//! 座標系は PDF と同じ(原点は左下、単位は pt)。

use crate::fonts::{self, FontId};

pub const MM: f32 = 72.0 / 25.4;

/// CMYK の色(各成分 0〜1)。このアプリで使える色はこれだけ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cmyk(pub f32, pub f32, pub f32, pub f32);

impl Cmyk {
    pub const PAPER: Cmyk = Cmyk(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Cmyk = Cmyk(0.0, 0.0, 0.0, 1.0);
    /// トンボ用のレジストレーション
    pub const REGISTRATION: Cmyk = Cmyk(1.0, 1.0, 1.0, 1.0);

    /// 総インキ量(%).
    pub fn total_ink(&self) -> f32 {
        (self.0 + self.1 + self.2 + self.3) * 100.0
    }

    /// 画面表示用の近似 RGB.
    pub fn approx_hex(&self) -> String {
        let ch = |v: f32| (255.0 * (1.0 - v) * (1.0 - self.3)).round() as u8;
        format!("#{:02x}{:02x}{:02x}", ch(self.0), ch(self.1), ch(self.2))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Curve(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Debug, Clone, Default)]
pub struct Path {
    pub segs: Vec<Seg>,
}

impl Path {
    pub fn new() -> Path {
        Path::default()
    }
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.segs.push(Seg::Move(x, y));
        self
    }
    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.segs.push(Seg::Line(x, y));
        self
    }
    pub fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) -> &mut Self {
        self.segs.push(Seg::Curve(x1, y1, x2, y2, x, y));
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.segs.push(Seg::Close);
        self
    }
}

#[derive(Debug, Clone)]
pub enum Op {
    Save,
    Restore,
    /// 現在の座標系に行列を掛ける(PDF の cm と同じ)
    Transform([f32; 6]),
    Fill(Cmyk),
    Stroke(Cmyk),
    LineWidth(f32),
    Path {
        path: Path,
        fill: bool,
        stroke: bool,
    },
    Text {
        font: FontId,
        size: f32,
        x: f32,
        y: f32,
        text: String,
    },
    /// 以降の描画を矩形で切り抜く(Restore で解除)
    ClipRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

pub type Rect = [f32; 4];

#[derive(Debug, Clone)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub trim: Rect,
    pub bleed: Rect,
    pub ops: Vec<Op>,
}

/// 1 ページ分の描画先.
pub struct Canvas {
    page: Page,
    font: FontId,
    size: f32,
}

impl Canvas {
    pub fn new(width: f32, height: f32, trim: Rect, bleed: Rect) -> Canvas {
        let mut c = Canvas {
            page: Page {
                width,
                height,
                trim,
                bleed,
                ops: Vec::new(),
            },
            font: FontId::Mincho,
            size: 10.0,
        };
        // 初期色も明示的に CMYK で指定する(既定の DeviceGray を使わない)
        c.set_fill(Cmyk::BLACK);
        c.set_stroke(Cmyk::BLACK);
        c
    }

    pub fn finish(self) -> Page {
        self.page
    }

    fn op(&mut self, op: Op) {
        self.page.ops.push(op);
    }

    pub fn save(&mut self) {
        self.op(Op::Save);
    }
    pub fn restore(&mut self) {
        self.op(Op::Restore);
    }
    pub fn translate(&mut self, x: f32, y: f32) {
        self.op(Op::Transform([1.0, 0.0, 0.0, 1.0, x, y]));
    }
    /// 反時計回りに deg 度回転.
    pub fn rotate(&mut self, deg: f32) {
        let (s, c) = deg.to_radians().sin_cos();
        self.op(Op::Transform([c, s, -s, c, 0.0, 0.0]));
    }
    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.op(Op::Transform([sx, 0.0, 0.0, sy, 0.0, 0.0]));
    }
    pub fn set_fill(&mut self, c: Cmyk) {
        self.op(Op::Fill(c));
    }
    pub fn set_stroke(&mut self, c: Cmyk) {
        self.op(Op::Stroke(c));
    }
    pub fn set_line_width(&mut self, w: f32) {
        self.op(Op::LineWidth(w));
    }

    /// 次の restore までの描画を矩形の内側に切り抜く.
    pub fn clip_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.op(Op::ClipRect { x, y, w, h });
    }

    pub fn draw_path(&mut self, path: Path, stroke: bool, fill: bool) {
        self.op(Op::Path { path, fill, stroke });
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, stroke: bool, fill: bool) {
        let mut p = Path::new();
        p.move_to(x, y)
            .line_to(x + w, y)
            .line_to(x + w, y + h)
            .line_to(x, y + h)
            .close();
        self.draw_path(p, stroke, fill);
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let mut p = Path::new();
        p.move_to(x1, y1).line_to(x2, y2);
        self.draw_path(p, true, false);
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, stroke: bool, fill: bool) {
        let k = 0.552_284_8 * r;
        let mut p = Path::new();
        p.move_to(cx + r, cy)
            .curve_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r)
            .curve_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy)
            .curve_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r)
            .curve_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy)
            .close();
        self.draw_path(p, stroke, fill);
    }

    pub fn set_font(&mut self, font: FontId, size: f32) {
        self.font = font;
        self.size = size;
    }

    pub fn draw_string(&mut self, x: f32, y: f32, text: &str) {
        if text.is_empty() {
            return;
        }
        let (font, size) = (self.font, self.size);
        self.op(Op::Text {
            font,
            size,
            x,
            y,
            text: text.to_string(),
        });
    }

    pub fn draw_centred(&mut self, x: f32, y: f32, text: &str) {
        let w = fonts::text_width(self.font, text, self.size);
        self.draw_string(x - w / 2.0, y, text);
    }

    pub fn draw_right(&mut self, x: f32, y: f32, text: &str) {
        let w = fonts::text_width(self.font, text, self.size);
        self.draw_string(x - w, y, text);
    }
}
