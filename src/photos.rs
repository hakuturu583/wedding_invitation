//! 案内図に入れる写真の保管と CMYK 変換.
//!
//! ブラウザで縮小・デコードした RGBA を受け取り、CMYK に変換して JPEG(DCTDecode)で保持する。
//! 画面プレビューには、ブラウザ側で作った JPEG の data URL をそのまま使う。

use std::cell::RefCell;
use std::collections::HashMap;

use jpeg_encoder::{ColorType, Encoder};

/// 埋め込む写真の長辺の上限(px)。ハガキ全面でも 350dpi 程度になる大きさ.
pub const MAX_EDGE: u32 = 2000;
const JPEG_QUALITY: u8 = 90;

#[derive(Debug, Clone)]
pub struct Photo {
    pub width: u32,
    pub height: u32,
    /// CMYK の JPEG(Adobe 方式で値は反転して保存されている)
    pub cmyk_jpeg: Vec<u8>,
    /// 画面プレビュー用の data URL
    pub preview_url: String,
}

thread_local! {
    static PHOTOS: RefCell<HashMap<String, Photo>> = RefCell::new(HashMap::new());
}

/// sRGB(0〜255)を CMYK(0〜255)に変換する.
///
/// 墨版を最大にとる簡易変換(GCR 100%)。色版の最大値は必ず 0 になるので、
/// 総インキ量は常に 300% 以下に収まる。厳密な色合わせが必要な場合は印刷所のプロファイルで変換する。
pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> [u8; 4] {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 {
        return [0, 0, 0, 255];
    }
    let f = |v: f32| (((1.0 - v - k) / (1.0 - k)).clamp(0.0, 1.0) * 255.0).round() as u8;
    [f(r), f(g), f(b), (k * 255.0).round() as u8]
}

/// RGBA(透明部分は白とみなす)を CMYK の JPEG にする.
pub fn encode_cmyk_jpeg(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || width > MAX_EDGE * 2 || height > MAX_EDGE * 2 {
        return Err("写真の大きさが正しくありません".into());
    }
    if rgba.len() != (width * height * 4) as usize {
        return Err("写真のデータが壊れています".into());
    }
    let mut cmyk = Vec::with_capacity(rgba.len());
    for px in rgba.as_chunks::<4>().0 {
        let a = px[3] as f32 / 255.0;
        let over_white = |v: u8| (v as f32 * a + 255.0 * (1.0 - a)).round() as u8;
        cmyk.extend_from_slice(&rgb_to_cmyk(over_white(px[0]), over_white(px[1]), over_white(px[2])));
    }
    let mut out = Vec::new();
    Encoder::new(&mut out, JPEG_QUALITY)
        .encode(&cmyk, width as u16, height as u16, ColorType::Cmyk)
        .map_err(|e| format!("写真を変換できません: {e}"))?;
    Ok(out)
}

/// 写真を登録する(同じ ID なら置き換え).
pub fn add(id: &str, width: u32, height: u32, rgba: &[u8], preview_url: String) -> Result<(), String> {
    let cmyk_jpeg = encode_cmyk_jpeg(width, height, rgba)?;
    PHOTOS.with(|p| {
        p.borrow_mut().insert(
            id.to_string(),
            Photo {
                width,
                height,
                cmyk_jpeg,
                preview_url,
            },
        )
    });
    Ok(())
}

pub fn remove(id: &str) {
    PHOTOS.with(|p| p.borrow_mut().remove(id));
}

pub fn get(id: &str) -> Option<Photo> {
    if id.is_empty() {
        return None;
    }
    PHOTOS.with(|p| p.borrow().get(id).cloned())
}

pub fn has(id: &str) -> bool {
    PHOTOS.with(|p| p.borrow().contains_key(id))
}

/// 動作確認用の合成画像(RGBA)。kind 0 は地図風、それ以外は街角の写真風.
#[doc(hidden)]
pub fn demo_rgba(width: u32, height: u32, kind: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let (fx, fy) = (x as f32 / width as f32, y as f32 / height as f32);
            let px: [u8; 3] = if kind == 0 {
                // 地図: 生成り色の地に道路と川、赤い目印
                let road = (fx - 0.35).abs() < 0.018 || (fy - 0.55).abs() < 0.02 || ((fx + fy) - 1.1).abs() < 0.015;
                let river = ((fy - 0.25) - 0.08 * (fx * 9.0).sin()).abs() < 0.03;
                let marker = ((fx - 0.62).powi(2) + (fy - 0.42).powi(2)).sqrt() < 0.035;
                if marker {
                    [220, 40, 50]
                } else if road {
                    [150, 150, 150]
                } else if river {
                    [120, 180, 230]
                } else {
                    [245, 240, 225]
                }
            } else {
                // 写真: 空・建物・道・看板
                let hue = (kind * 47 % 100) as f32 / 100.0;
                if fy < 0.35 {
                    [(120.0 + 60.0 * fy) as u8, (170.0 + 50.0 * fy) as u8, 235]
                } else if fy > 0.8 {
                    [110, 110, 115]
                } else if ((fx - 0.25).abs() < 0.08) && fy > 0.45 && fy < 0.6 {
                    [210, 50, 40]
                } else if fx > 0.55 {
                    [(180.0 * hue + 60.0) as u8, 150, (200.0 * (1.0 - hue)) as u8]
                } else {
                    [60, (120.0 + 60.0 * fx) as u8, 70]
                }
            };
            out.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion() {
        assert_eq!(rgb_to_cmyk(255, 255, 255), [0, 0, 0, 0]);
        assert_eq!(rgb_to_cmyk(0, 0, 0), [0, 0, 0, 255]);
        assert_eq!(rgb_to_cmyk(255, 0, 0), [0, 255, 255, 0]);
        assert_eq!(rgb_to_cmyk(128, 128, 128)[..3], [0, 0, 0], "グレーは墨だけ");
        for (r, g, b) in [(10, 200, 30), (0, 0, 128), (90, 40, 200), (3, 3, 2)] {
            let total: u32 = rgb_to_cmyk(r, g, b).iter().map(|v| *v as u32).sum();
            assert!(total <= 255 * 3, "総インキ量 300% 以下");
        }
    }

    #[test]
    fn store() {
        let rgba = [255u8, 0, 0, 255].repeat(16);
        add("t", 4, 4, &rgba, "data:".into()).unwrap();
        let p = get("t").unwrap();
        assert_eq!((p.width, p.height), (4, 4));
        assert_eq!(&p.cmyk_jpeg[..2], &[0xFF, 0xD8]);
        remove("t");
        assert!(!has("t"));
        assert!(add("bad", 4, 4, &[0; 3], String::new()).is_err());
    }
}
