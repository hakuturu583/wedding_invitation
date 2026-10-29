//! ディスプレイリストを画面プレビュー用の SVG に書き出す.
//!
//! 色は CMYK から近似した RGB。文字はページ側で登録した Web フォントで表示し、
//! 1 文字ずつ PDF と同じ送り幅で配置する。

use std::fmt::Write;

use crate::canvas::{Op, Page, Seg};
use crate::fonts;

type Matrix = [f32; 6];

/// outer ∘ inner(inner を先に適用).
fn mul(o: &Matrix, i: &Matrix) -> Matrix {
    [
        o[0] * i[0] + o[2] * i[1],
        o[1] * i[0] + o[3] * i[1],
        o[0] * i[2] + o[2] * i[3],
        o[1] * i[2] + o[3] * i[3],
        o[0] * i[4] + o[2] * i[5] + o[4],
        o[1] * i[4] + o[3] * i[5] + o[5],
    ]
}

fn fmt_matrix(m: &Matrix) -> String {
    format!(
        "matrix({:.4} {:.4} {:.4} {:.4} {:.3} {:.3})",
        m[0], m[1], m[2], m[3], m[4], m[5]
    )
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

struct State {
    ctm: Matrix,
    /// この状態で開いた切り抜き用の <g> の数
    clips: usize,
    fill: String,
    stroke: String,
    line_width: f32,
}

pub fn write_svg(page: &Page) -> String {
    let (w, h) = (page.width, page.height);
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.2} {h:.2}" width="{w:.2}pt" height="{h:.2}pt">"#
    );
    let _ = write!(out, r##"<rect width="{w:.2}" height="{h:.2}" fill="#ffffff"/>"##);
    let mut stack = Vec::new();
    let mut clip_id = 0;
    let mut st = State {
        ctm: [1.0, 0.0, 0.0, -1.0, 0.0, h],
        clips: 0,
        fill: "#000000".into(),
        stroke: "#000000".into(),
        line_width: 1.0,
    };
    for op in &page.ops {
        match op {
            Op::Save => {
                stack.push(State {
                    fill: st.fill.clone(),
                    stroke: st.stroke.clone(),
                    ..st
                });
                st.clips = 0;
            }
            Op::Restore => {
                out.push_str(&"</g>".repeat(st.clips));
                if let Some(s) = stack.pop() {
                    st = s;
                }
            }
            Op::Image { id, x, y, w, h } => {
                let Some(photo) = crate::photos::get(id) else { continue };
                // 画像の上端が y + h に来るよう、単位正方形を上下反転して置く
                let m = mul(&st.ctm, &[*w, 0.0, 0.0, -*h, *x, y + h]);
                let _ = write!(
                    out,
                    r#"<image transform="{}" width="1" height="1" preserveAspectRatio="none" href="{}"/>"#,
                    fmt_matrix(&m),
                    escape(&photo.preview_url)
                );
            }
            Op::ClipRect { x, y, w, h } => {
                // 現在の座標系での矩形を SVG の座標に直す(回転は使わない前提で外接矩形を取る)
                let m = st.ctm;
                let pts = [(*x, *y), (x + w, *y), (*x, y + h), (x + w, y + h)]
                    .map(|(px, py)| (m[0] * px + m[2] * py + m[4], m[1] * px + m[3] * py + m[5]));
                let x0 = pts.iter().map(|p| p.0).fold(f32::MAX, f32::min);
                let x1 = pts.iter().map(|p| p.0).fold(f32::MIN, f32::max);
                let y0 = pts.iter().map(|p| p.1).fold(f32::MAX, f32::min);
                let y1 = pts.iter().map(|p| p.1).fold(f32::MIN, f32::max);
                clip_id += 1;
                let _ = write!(
                    out,
                    r#"<clipPath id="clip{clip_id}"><rect x="{x0:.3}" y="{y0:.3}" width="{:.3}" height="{:.3}"/></clipPath><g clip-path="url(#clip{clip_id})">"#,
                    x1 - x0,
                    y1 - y0
                );
                st.clips += 1;
            }
            Op::Transform(m) => st.ctm = mul(&st.ctm, m),
            Op::Fill(c) => st.fill = c.approx_hex(),
            Op::Stroke(c) => st.stroke = c.approx_hex(),
            Op::LineWidth(lw) => st.line_width = *lw,
            Op::Path { path, fill, stroke } => {
                let mut d = String::new();
                for seg in &path.segs {
                    let _ = match *seg {
                        Seg::Move(x, y) => write!(d, "M{x:.3} {y:.3}"),
                        Seg::Line(x, y) => write!(d, "L{x:.3} {y:.3}"),
                        Seg::Curve(x1, y1, x2, y2, x, y) => {
                            write!(d, "C{x1:.3} {y1:.3} {x2:.3} {y2:.3} {x:.3} {y:.3}")
                        }
                        Seg::Close => write!(d, "Z"),
                    };
                }
                let fill_attr = if *fill { st.fill.as_str() } else { "none" };
                let _ = write!(
                    out,
                    r#"<path d="{d}" transform="{}" fill="{fill_attr}""#,
                    fmt_matrix(&st.ctm)
                );
                if *stroke {
                    let _ = write!(out, r#" stroke="{}" stroke-width="{:.3}""#, st.stroke, st.line_width);
                }
                out.push_str("/>");
            }
            Op::Text { font, size, x, y, text } => {
                let m = mul(&st.ctm, &[1.0, 0.0, 0.0, -1.0, *x, *y]);
                let mut pos = 0.0;
                let xs: Vec<String> = fonts::advances(*font, text, *size)
                    .into_iter()
                    .map(|adv| {
                        let s = format!("{pos:.2}");
                        pos += adv;
                        s
                    })
                    .collect();
                let _ = write!(
                    out,
                    r#"<text xml:space="preserve" style="white-space:pre" transform="{}" x="{}" y="0" font-family="{}" font-size="{size:.3}" fill="{}">{}</text>"#,
                    fmt_matrix(&m),
                    xs.join(" "),
                    font.css_family(),
                    st.fill,
                    escape(text)
                );
            }
        }
    }
    out.push_str(&"</g>".repeat(st.clips + stack.iter().map(|s: &State| s.clips).sum::<usize>()));
    out.push_str("</svg>");
    out
}
