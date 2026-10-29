//! 結婚式招待状の印刷入稿用 PDF (CMYK) を生成する WebAssembly アプリ.
//!
//! レイアウトは [`canvas::Canvas`] に描画命令として記録し、入稿用の PDF と
//! 画面プレビュー用の SVG の両方に同じ内容を書き出す。

pub mod canvas;
pub mod csv;
pub mod designs;
pub mod fonts;
pub mod layout;
pub mod model;
pub mod pdf;
pub mod roster;
pub mod svg;
pub mod typeset;
pub mod xlsx;
pub mod zip;

use wasm_bindgen::prelude::*;

use model::{Event, Options, Order};

pub const CARD_FILE: &str = "invitation_cards.pdf";
pub const ENVELOPE_FILE: &str = "envelopes.pdf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doc {
    Card,
    Envelope,
}

impl Doc {
    pub fn parse(s: &str) -> Result<Doc, String> {
        match s {
            "card" => Ok(Doc::Card),
            "envelope" => Ok(Doc::Envelope),
            _ => Err("不明な出力種別です".into()),
        }
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
    }
}

pub fn build_pdf(order: &Order, doc: Doc) -> Result<Vec<u8>, String> {
    ensure_fonts()?;
    let d = designs::get(&order.design);
    let title = match doc {
        Doc::Card => format!("招待状カード ({})", d.name),
        Doc::Envelope => format!("封筒 宛名 ({})", d.name),
    };
    pdf::write_pdf(&pages(order, doc), &title)
}

pub fn build_zip(order: &Order) -> Result<Vec<u8>, String> {
    let card = build_pdf(order, Doc::Card)?;
    let envelope = build_pdf(order, Doc::Envelope)?;
    Ok(zip::write(&[(CARD_FILE, &card), (ENVELOPE_FILE, &envelope)], false))
}

/// 指定したゲスト 1 名分の 1 ページ目を SVG で返す.
pub fn build_preview(order: &Order, doc: Doc, guest: usize) -> Result<String, String> {
    ensure_fonts()?;
    let guest = order.guests[guest.min(order.guests.len() - 1)].clone();
    let one = Order {
        guests: vec![guest],
        options: Options {
            envelope_back: false,
            ..order.options
        },
        ..order.clone()
    };
    Ok(svg::write_svg(&pages(&one, doc)[0]))
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

/// 入稿用 PDF を生成する。doc は "card" か "envelope".
#[wasm_bindgen(js_name = renderPdf)]
pub fn render_pdf(order_json: &str, doc: &str) -> Result<Vec<u8>, JsError> {
    let order = Order::from_json(order_json).map_err(js_err)?;
    build_pdf(&order, Doc::parse(doc).map_err(js_err)?).map_err(js_err)
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
    let order = Order::from_json(order_json).map_err(js_err)?;
    build_preview(&order, Doc::parse(doc).map_err(js_err)?, guest).map_err(js_err)
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
