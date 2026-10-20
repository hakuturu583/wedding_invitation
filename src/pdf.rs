//! ディスプレイリストを入稿用 PDF に書き出す.
//!
//! - 色はすべて DeviceCMYK (`k` / `K` オペレータのみ)。RGB・グレー・透明効果は出力しない
//! - フォントは TrueType のサブセットを埋め込む(Type0 / CIDFontType2, Identity-H)
//! - ページには MediaBox / TrimBox / BleedBox を設定する

use std::collections::BTreeMap;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use subsetter::GlyphRemapper;

use crate::canvas::{Op, Page, Seg};
use crate::fonts::{self, FontId};

#[derive(Default)]
struct FontUse {
    remapper: GlyphRemapper,
    /// 新しいグリフ ID -> (元のグリフ ID, 文字)
    glyphs: BTreeMap<u16, (u16, char)>,
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

fn resource_name(font: FontId) -> Name<'static> {
    match font {
        FontId::Mincho => Name(b"F1"),
        FontId::Script => Name(b"F2"),
    }
}

fn pdf_rect(r: [f32; 4]) -> Rect {
    Rect::new(r[0], r[1], r[2], r[3])
}

fn encode_text(font: FontId, text: &str, uses: &mut BTreeMap<FontId, FontUse>) -> Vec<u8> {
    let loaded = fonts::get(font);
    let usage = uses.entry(font).or_default();
    let mut out = Vec::with_capacity(text.len() * 2);
    for ch in text.chars() {
        let gid = loaded.glyph(ch);
        let new = usage.remapper.remap(gid);
        usage.glyphs.entry(new).or_insert((gid, ch));
        out.extend_from_slice(&new.to_be_bytes());
    }
    out
}

fn content_stream(page: &Page, uses: &mut BTreeMap<FontId, FontUse>) -> Vec<u8> {
    let mut c = Content::new();
    for op in &page.ops {
        match op {
            Op::Save => {
                c.save_state();
            }
            Op::Restore => {
                c.restore_state();
            }
            Op::Transform(m) => {
                c.transform(*m);
            }
            Op::Fill(col) => {
                c.set_fill_cmyk(col.0, col.1, col.2, col.3);
            }
            Op::Stroke(col) => {
                c.set_stroke_cmyk(col.0, col.1, col.2, col.3);
            }
            Op::LineWidth(w) => {
                c.set_line_width(*w);
            }
            Op::Path { path, fill, stroke } => {
                for seg in &path.segs {
                    match *seg {
                        Seg::Move(x, y) => c.move_to(x, y),
                        Seg::Line(x, y) => c.line_to(x, y),
                        Seg::Curve(x1, y1, x2, y2, x, y) => c.cubic_to(x1, y1, x2, y2, x, y),
                        Seg::Close => c.close_path(),
                    };
                }
                match (fill, stroke) {
                    (true, true) => c.fill_nonzero_and_stroke(),
                    (true, false) => c.fill_nonzero(),
                    (false, true) => c.stroke(),
                    (false, false) => c.end_path(),
                };
            }
            Op::Text { font, size, x, y, text } => {
                let bytes = encode_text(*font, text, uses);
                c.begin_text();
                c.set_font(resource_name(*font), *size);
                c.set_text_matrix([1.0, 0.0, 0.0, 1.0, *x, *y]);
                c.show(Str(&bytes));
                c.end_text();
            }
        }
    }
    c.finish().into_vec()
}

/// サブセットのタグ(例: "ABCDEF+")。収録グリフから決める.
fn subset_tag(usage: &FontUse) -> String {
    let mut h: u32 = 2166136261;
    for (old, _) in usage.glyphs.values() {
        h = (h ^ *old as u32).wrapping_mul(16777619);
    }
    (0..6).map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char).collect()
}

pub fn write_pdf(pages: &[Page], title: &str) -> Result<Vec<u8>, String> {
    let mut pdf = Pdf::new();
    pdf.set_version(1, 4);
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog_id = alloc();
    let tree_id = alloc();
    let info_id = alloc();
    let font_ids: BTreeMap<FontId, Ref> = FontId::ALL.iter().map(|f| (*f, alloc())).collect();

    let mut uses: BTreeMap<FontId, FontUse> = BTreeMap::new();
    let mut page_ids = Vec::new();
    for page in pages {
        let page_id = alloc();
        let content_id = alloc();
        page_ids.push(page_id);
        let stream = content_stream(page, &mut uses);
        pdf.stream(content_id, &deflate(&stream)).filter(Filter::FlateDecode);
        let mut p = pdf.page(page_id);
        p.media_box(Rect::new(0.0, 0.0, page.width, page.height));
        p.trim_box(pdf_rect(page.trim));
        p.bleed_box(pdf_rect(page.bleed));
        p.parent(tree_id);
        p.contents(content_id);
        let mut res = p.resources();
        let mut fonts_dict = res.fonts();
        for (font, id) in &font_ids {
            fonts_dict.pair(resource_name(*font), *id);
        }
    }

    for (font, type0_id) in &font_ids {
        let usage = uses.remove(font).unwrap_or_default();
        let loaded = fonts::get(*font);
        let cid_id = alloc();
        let descriptor_id = alloc();
        let file_id = alloc();
        let cmap_id = alloc();
        let base = format!("{}+{}", subset_tag(&usage), font.postscript_name());
        let base_name = Name(base.as_bytes());
        let system_info = SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"Identity"),
            supplement: 0,
        };

        pdf.type0_font(*type0_id)
            .base_font(base_name)
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid_id)
            .to_unicode(cmap_id);

        let count = usage.remapper.num_gids();
        let widths: Vec<f32> = (0..count)
            .map(|new| usage.glyphs.get(&new).map(|(old, _)| *old).unwrap_or(0))
            .map(|old| loaded.advance_1000(old))
            .collect();
        let mut cid = pdf.cid_font(cid_id);
        cid.subtype(CidFontType::Type2)
            .base_font(base_name)
            .system_info(system_info)
            .font_descriptor(descriptor_id)
            .default_width(1000.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        cid.widths().consecutive(0, widths);
        cid.finish();

        let face = &loaded.face;
        let bbox = face.global_bounding_box();
        let to = |v: i16| loaded.units_to_1000(v);
        let mut flags = FontFlags::SYMBOLIC;
        if *font == FontId::Mincho {
            flags |= FontFlags::SERIF;
        } else {
            flags |= FontFlags::SCRIPT;
        }
        pdf.font_descriptor(descriptor_id)
            .name(base_name)
            .flags(flags)
            .bbox(Rect::new(
                to(bbox.x_min),
                to(bbox.y_min),
                to(bbox.x_max),
                to(bbox.y_max),
            ))
            .italic_angle(0.0)
            .ascent(to(face.ascender()))
            .descent(to(face.descender()))
            .cap_height(to(face.capital_height().unwrap_or(face.ascender())))
            .stem_v(80.0)
            .font_file2(file_id);

        let subset = subsetter::subset(loaded.data, 0, &usage.remapper)
            .map_err(|e| format!("フォントのサブセット化に失敗しました: {e:?}"))?;
        pdf.stream(file_id, &deflate(&subset))
            .filter(Filter::FlateDecode)
            .pair(Name(b"Length1"), subset.len() as i32);

        let mut cmap = pdf_writer::types::UnicodeCmap::new(Name(b"Custom"), system_info);
        for (new, (_, ch)) in &usage.glyphs {
            cmap.pair(*new, *ch);
        }
        pdf.stream(cmap_id, &cmap.finish());
    }

    pdf.pages(tree_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.document_info(info_id)
        .title(TextStr(title))
        .subject(TextStr("印刷入稿用データ (CMYK)"))
        .creator(TextStr("Wedding Invitation Maker"))
        .producer(TextStr("wedding_invitation (Rust / WebAssembly)"));
    Ok(pdf.finish())
}
