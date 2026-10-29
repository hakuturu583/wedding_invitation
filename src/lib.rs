//! 結婚式招待状の印刷入稿用 PDF (CMYK) を生成する WebAssembly アプリ.
//!
//! レイアウトは [`canvas::Canvas`] に描画命令として記録し、入稿用の PDF と
//! 画面プレビュー用の SVG の両方に同じ内容を書き出す。

pub mod access;
pub mod canvas;
pub mod csv;
pub mod designs;
pub mod fonts;
pub mod layout;
pub mod model;
pub mod music;
pub mod pdf;
pub mod photos;
pub mod roster;
pub mod svg;
pub mod typeset;
pub mod xlsx;
pub mod zip;

use wasm_bindgen::prelude::*;

use model::{Event, Options, Order};

pub const CARD_FILE: &str = "invitation_cards.pdf";
pub const ENVELOPE_FILE: &str = "envelopes.pdf";
pub const ACCESS_FILE: &str = "access_map.pdf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doc {
    Card,
    Envelope,
    /// 案内図(ハガキ)
    Access,
}

impl Doc {
    pub fn parse(s: &str) -> Result<Doc, String> {
        match s {
            "card" => Ok(Doc::Card),
            "envelope" => Ok(Doc::Envelope),
            "access" => Ok(Doc::Access),
            _ => Err("不明な出力種別です".into()),
        }
    }

    /// 宛名(ゲスト)が必要な出力か.
    pub fn needs_guests(self) -> bool {
        self != Doc::Access
    }
}

fn parse_order(order_json: &str, doc: Doc) -> Result<Order, String> {
    if doc.needs_guests() {
        Order::from_json(order_json)
    } else {
        Order::from_json_without_guests(order_json)
    }
}

fn ensure_fonts() -> Result<(), String> {
    if fonts::is_ready() {
        Ok(())
    } else {
        Err("フォントが読み込まれていません".into())
    }
}

pub fn pages(order: &Order, doc: Doc) -> Vec<canvas::Page> {
    match doc {
        Doc::Card => layout::card_pages(&order.event, &order.guests, &order.design, &order.options),
        Doc::Envelope => layout::envelope_pages(&order.event, &order.guests, &order.design, &order.options),
        Doc::Access => access::access_pages(&order.event, &order.access, &order.design, &order.options),
    }
}

pub fn build_pdf(order: &Order, doc: Doc) -> Result<Vec<u8>, String> {
    ensure_fonts()?;
    let d = designs::get(&order.design);
    let title = match doc {
        Doc::Card => format!("招待状カード ({})", d.name),
        Doc::Envelope => format!("封筒 宛名 ({})", d.name),
        Doc::Access => format!("案内図 ({})", d.name),
    };
    pdf::write_pdf(&pages(order, doc), &title)
}

pub fn build_zip(order: &Order) -> Result<Vec<u8>, String> {
    let card = build_pdf(order, Doc::Card)?;
    let envelope = build_pdf(order, Doc::Envelope)?;
    let mut files: Vec<(&str, &[u8])> = vec![(CARD_FILE, &card), (ENVELOPE_FILE, &envelope)];
    // 案内図は内容があるときだけ入れる
    let access = if order.access.is_empty() {
        None
    } else {
        Some(build_pdf(order, Doc::Access)?)
    };
    if let Some(access) = &access {
        files.push((ACCESS_FILE, access));
    }
    Ok(zip::write(&files, false))
}

/// 指定したゲスト 1 名分の 1 ページ目を SVG で返す(案内図はゲストに関係なく、表裏の全ページ).
pub fn build_preview(order: &Order, doc: Doc, guest: usize) -> Result<String, String> {
    ensure_fonts()?;
    let guests = match order.guests.get(guest.min(order.guests.len().saturating_sub(1))) {
        Some(g) => vec![g.clone()],
        None if !doc.needs_guests() => Vec::new(),
        None => return Err("ゲストを 1 名以上入力してください".into()),
    };
    let one = Order {
        guests,
        options: Options {
            envelope_back: false,
            ..order.options
        },
        ..order.clone()
    };
    let pages = pages(&one, doc);
    if doc == Doc::Access {
        // 案内図は表裏をまとめて返す
        return Ok(pages.iter().map(svg::write_svg).collect::<Vec<_>>().join("\n"));
    }
    Ok(svg::write_svg(&pages[0]))
}

// ------------------------------------------------------------------ JS 向け API

fn js_err(e: String) -> JsError {
    JsError::new(&e)
}

/// フォント(TrueType)のバイト列を登録する。最初に 1 回呼ぶ.
#[wasm_bindgen(js_name = initFonts)]
pub fn init_fonts(mincho: Vec<u8>, script: Vec<u8>) -> Result<(), JsError> {
    fonts::init(mincho, script).map_err(js_err)
}

/// デザイン一覧(JSON).
#[wasm_bindgen(js_name = designsJson)]
pub fn designs_json() -> String {
    let infos: Vec<_> = designs::DESIGNS.iter().map(|d| d.info()).collect();
    serde_json::to_string(&infos).unwrap_or_default()
}

/// 挙式情報の初期値(JSON).
#[wasm_bindgen(js_name = defaultEventJson)]
pub fn default_event_json() -> String {
    serde_json::to_string(&Event::default()).unwrap_or_default()
}

#[wasm_bindgen(js_name = maxGuests)]
pub fn max_guests() -> usize {
    model::MAX_GUESTS
}

/// 入稿用 PDF を生成する。doc は "card" / "envelope" / "access".
#[wasm_bindgen(js_name = renderPdf)]
pub fn render_pdf(order_json: &str, doc: &str) -> Result<Vec<u8>, JsError> {
    let doc = Doc::parse(doc).map_err(js_err)?;
    let order = parse_order(order_json, doc).map_err(js_err)?;
    build_pdf(&order, doc).map_err(js_err)
}

/// 招待状カードと封筒の PDF をまとめた ZIP を生成する.
#[wasm_bindgen(js_name = renderZip)]
pub fn render_zip(order_json: &str) -> Result<Vec<u8>, JsError> {
    let order = Order::from_json(order_json).map_err(js_err)?;
    build_zip(&order).map_err(js_err)
}

/// 画面プレビュー用の SVG を生成する.
#[wasm_bindgen(js_name = renderPreview)]
pub fn render_preview(order_json: &str, doc: &str, guest: usize) -> Result<String, JsError> {
    let doc = Doc::parse(doc).map_err(js_err)?;
    let order = parse_order(order_json, doc).map_err(js_err)?;
    build_preview(&order, doc, guest).map_err(js_err)
}

/// 案内図の写真を登録する。rgba はブラウザでデコード・縮小した画素、preview は画面表示用の data URL.
#[wasm_bindgen(js_name = addPhoto)]
pub fn add_photo(id: &str, width: u32, height: u32, rgba: &[u8], preview: String) -> Result<(), JsError> {
    photos::add(id, width, height, rgba, preview).map_err(js_err)
}

#[wasm_bindgen(js_name = removePhoto)]
pub fn remove_photo(id: &str) {
    photos::remove(id);
}

/// 写真の長辺の上限(px).
#[wasm_bindgen(js_name = maxPhotoEdge)]
pub fn max_photo_edge() -> u32 {
    photos::MAX_EDGE
}

/// 案内図の初期値と交通手段の一覧(JSON).
#[wasm_bindgen(js_name = accessDefaultsJson)]
pub fn access_defaults_json() -> String {
    let modes: Vec<_> = model::ACCESS_MODES
        .iter()
        .map(|(k, l)| serde_json::json!({"key": k, "label": l}))
        .collect();
    serde_json::json!({
        "access": model::Access::default(),
        "modes": modes,
        "max_routes": model::MAX_ROUTES,
        "max_steps": model::MAX_STEPS,
    })
    .to_string()
}

/// CSV を読み込み、ゲスト一覧(JSON)を返す.
#[wasm_bindgen(js_name = parseGuestsCsv)]
pub fn parse_guests_csv(data: &[u8]) -> Result<String, JsError> {
    let guests = csv::parse_guests(data).map_err(js_err)?;
    Ok(serde_json::to_string(&guests).unwrap_or_default())
}

/// ゲスト名簿のひな形 CSV.
#[wasm_bindgen(js_name = sampleCsv)]
pub fn sample_csv() -> Vec<u8> {
    csv::sample_csv()
}

/// 現在の入力内容を Excel(xlsx)に書き出す。ゲストが 0 名でもよい.
#[wasm_bindgen(js_name = exportXlsx)]
pub fn export_xlsx(order_json: &str) -> Result<Vec<u8>, JsError> {
    let order: Order = serde_json::from_str(order_json).map_err(|e| JsError::new(&e.to_string()))?;
    let order = Order {
        event: order.event.cleaned(),
        access: order.access.cleaned(),
        guests: order
            .guests
            .into_iter()
            .map(model::Guest::cleaned)
            .filter(|g| !g.name.is_empty())
            .collect(),
        ..order
    };
    Ok(xlsx::write_workbook(&order))
}

fn guests_from_json(order_json: &str) -> Result<Vec<model::Guest>, JsError> {
    let order: Order = serde_json::from_str(order_json).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(order
        .guests
        .into_iter()
        .map(model::Guest::cleaned)
        .filter(|g| !g.name.is_empty())
        .collect())
}

/// ゲスト名簿を One-W の出席者名簿形式の xlsx にする.
#[wasm_bindgen(js_name = exportGuestsXlsx)]
pub fn export_guests_xlsx(order_json: &str) -> Result<Vec<u8>, JsError> {
    Ok(xlsx::write_guest_workbook(&guests_from_json(order_json)?))
}

/// ゲスト名簿を One-W の出席者名簿形式の CSV(BOM 付き UTF-8)にする.
#[wasm_bindgen(js_name = exportGuestsCsv)]
pub fn export_guests_csv(order_json: &str) -> Result<Vec<u8>, JsError> {
    Ok(csv::guests_csv(&guests_from_json(order_json)?))
}

/// サンプル入りの Excel テンプレート.
#[wasm_bindgen(js_name = templateXlsx)]
pub fn template_xlsx() -> Vec<u8> {
    xlsx::template()
}

/// Excel(xlsx)を読み込み、{order, has_settings, warnings} の JSON を返す.
/// has_settings が偽なら名簿だけのファイル(One-W の出席者名簿など).
#[wasm_bindgen(js_name = importXlsx)]
pub fn import_xlsx(data: &[u8]) -> Result<String, JsError> {
    let imported = xlsx::read_workbook(data).map_err(js_err)?;
    Ok(serde_json::to_string(&imported).unwrap_or_default())
}

// ------------------------------------------------------------------ 楽曲管理

/// 楽曲管理の初期データ(結婚式の主なシーンを選定中で並べた JSON).
#[wasm_bindgen(js_name = musicDefaultJson)]
pub fn music_default_json() -> String {
    music::to_json(&music::MusicList::default())
}

/// 楽曲管理の JSON を読み、整えた {list, warnings} を返す.
#[wasm_bindgen(js_name = musicNormalize)]
pub fn music_normalize(text: &str) -> Result<String, JsError> {
    let n = music::normalize(text).map_err(js_err)?;
    Ok(serde_json::to_string(&n).unwrap_or_default())
}

/// 楽曲一覧を保存用の JSON(整形済み)にする.
#[wasm_bindgen(js_name = musicToJson)]
pub fn music_to_json(list_json: &str) -> Result<String, JsError> {
    let n = music::normalize(list_json).map_err(js_err)?;
    Ok(music::to_json(&n.list))
}

/// 集計(状況ごと・発注先ごとの件数など)の JSON.
#[wasm_bindgen(js_name = musicSummary)]
pub fn music_summary(list_json: &str) -> Result<String, JsError> {
    let n = music::normalize(list_json).map_err(js_err)?;
    Ok(serde_json::to_string(&music::summary(&n.list)).unwrap_or_default())
}

/// 発注状況の一覧(進む順)と上限.
#[wasm_bindgen(js_name = musicConfigJson)]
pub fn music_config_json() -> String {
    serde_json::json!({
        "statuses": music::STATUSES,
        "max_songs": music::MAX_SONGS,
        "max_candidates": music::MAX_CANDIDATES,
    })
    .to_string()
}
