//! フォントの読み込みと文字幅の計測.
//!
//! フォントファイルは wasm に埋め込まず、ブラウザ側で取得したバイト列を
//! [`init`] で一度だけ登録する(wasm のサイズを小さく保つため)。

use std::sync::OnceLock;

use ttf_parser::{Face, GlyphId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FontId {
    /// 本文用の明朝体(しっぽり明朝)
    Mincho,
    /// 欧文タイトル用のスクリプト体(Great Vibes)
    Script,
}

impl FontId {
    pub const ALL: [FontId; 2] = [FontId::Mincho, FontId::Script];

    pub fn postscript_name(self) -> &'static str {
        match self {
            FontId::Mincho => "ShipporiMincho-Regular",
            FontId::Script => "GreatVibes-Regular",
        }
    }

    /// プレビュー(SVG)で使う CSS の font-family 名.
    pub fn css_family(self) -> &'static str {
        match self {
            FontId::Mincho => "ShipporiMincho",
            FontId::Script => "GreatVibes",
        }
    }
}

pub struct LoadedFont {
    pub data: &'static [u8],
    pub face: Face<'static>,
}

impl LoadedFont {
    fn parse(data: Vec<u8>, label: &str) -> Result<LoadedFont, String> {
        let data: &'static [u8] = Box::leak(data.into_boxed_slice());
        let face = Face::parse(data, 0).map_err(|e| format!("{label} フォントを読み込めません: {e}"))?;
        Ok(LoadedFont { data, face })
    }

    pub fn glyph(&self, ch: char) -> u16 {
        self.face.glyph_index(ch).map(|g| g.0).unwrap_or(0)
    }

    /// グリフの送り幅(1000 分率).
    pub fn advance_1000(&self, gid: u16) -> f32 {
        let adv = self.face.glyph_hor_advance(GlyphId(gid)).unwrap_or(0) as f32;
        adv * 1000.0 / self.face.units_per_em() as f32
    }

    pub fn units_to_1000(&self, v: i16) -> f32 {
        v as f32 * 1000.0 / self.face.units_per_em() as f32
    }
}

struct Fonts {
    mincho: LoadedFont,
    script: LoadedFont,
}

static FONTS: OnceLock<Fonts> = OnceLock::new();

/// フォントを登録する。2 回目以降の呼び出しは何もしない.
pub fn init(mincho: Vec<u8>, script: Vec<u8>) -> Result<(), String> {
    if FONTS.get().is_some() {
        return Ok(());
    }
    let fonts = Fonts {
        mincho: LoadedFont::parse(mincho, "明朝体")?,
        script: LoadedFont::parse(script, "欧文")?,
    };
    let _ = FONTS.set(fonts);
    Ok(())
}

pub fn is_ready() -> bool {
    FONTS.get().is_some()
}

pub fn get(font: FontId) -> &'static LoadedFont {
    let fonts = FONTS.get().expect("fonts::init が呼ばれていません");
    match font {
        FontId::Mincho => &fonts.mincho,
        FontId::Script => &fonts.script,
    }
}

/// 文字列の幅(pt).
pub fn text_width(font: FontId, text: &str, size: f32) -> f32 {
    let f = get(font);
    text.chars().map(|ch| f.advance_1000(f.glyph(ch))).sum::<f32>() * size / 1000.0
}

/// 各文字の送り幅(pt).
pub fn advances(font: FontId, text: &str, size: f32) -> Vec<f32> {
    let f = get(font);
    text.chars()
        .map(|ch| f.advance_1000(f.glyph(ch)) * size / 1000.0)
        .collect()
}

/// テスト用: リポジトリ同梱のフォントを読み込む.
#[cfg(not(target_arch = "wasm32"))]
pub fn init_from_repo() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/www/fonts/");
    let read = |name: &str| std::fs::read(format!("{dir}{name}")).expect("フォントファイルがありません");
    init(read("ShipporiMincho-Regular.ttf"), read("GreatVibes-Regular.ttf")).unwrap();
}
