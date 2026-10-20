//! 入力内容を xlsx(Excel)に書き出し、読み込む.
//!
//! 「設定」シートにデザイン・挙式情報・出力オプションを、「ゲスト」シートに名簿を置く。
//! 読み込みは見出し(項目名)で対応を取るので、行や列の順番が入れ替わっても読める。
//! Excel が日付・時刻・数値として保存したセルも文字列に直して取り込む。

use std::collections::HashMap;
use std::fmt::Write;

use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use serde::Serialize;

use crate::designs;
use crate::model::{Date, Event, Guest, Options, Order, MAX_GUESTS};
use crate::zip;

pub const SETTINGS_SHEET: &str = "設定";
pub const GUEST_SHEET: &str = "ゲスト";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Date,
    Time,
    Multiline,
    /// する/しない
    Flag,
    Design,
}

#[derive(Clone, Copy)]
enum Slot {
    Design,
    Event(fn(&mut Event) -> &mut String),
    CropMarks,
    EnvelopeBack,
}

struct SettingRow {
    label: &'static str,
    kind: Kind,
    slot: Slot,
    help: &'static str,
}

const SETTINGS: &[SettingRow] = &[
    SettingRow {
        label: "デザイン",
        kind: Kind::Design,
        slot: Slot::Design,
        help: "プルダウンから選択",
    },
    SettingRow {
        label: "新郎のお名前",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.groom_name),
        help: "姓と名の間にスペース",
    },
    SettingRow {
        label: "新婦のお名前",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.bride_name),
        help: "姓と名の間にスペース",
    },
    SettingRow {
        label: "挙式日",
        kind: Kind::Date,
        slot: Slot::Event(|e| &mut e.date),
        help: "例: 2027-04-10 / 2027/4/10 / 2027年4月10日",
    },
    SettingRow {
        label: "受付開始",
        kind: Kind::Time,
        slot: Slot::Event(|e| &mut e.reception_open_time),
        help: "例: 10:30",
    },
    SettingRow {
        label: "挙式",
        kind: Kind::Time,
        slot: Slot::Event(|e| &mut e.ceremony_time),
        help: "例: 11:00",
    },
    SettingRow {
        label: "披露宴",
        kind: Kind::Time,
        slot: Slot::Event(|e| &mut e.party_time),
        help: "例: 12:00",
    },
    SettingRow {
        label: "返信期日",
        kind: Kind::Date,
        slot: Slot::Event(|e| &mut e.rsvp_deadline),
        help: "例: 2027-03-10",
    },
    SettingRow {
        label: "差出日の表記",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.date_line),
        help: "空欄なら挙式日の約2か月前の「〇年〇月吉日」",
    },
    SettingRow {
        label: "会場名",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.venue_name),
        help: "",
    },
    SettingRow {
        label: "会場住所",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.venue_address),
        help: "",
    },
    SettingRow {
        label: "会場電話番号",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.venue_tel),
        help: "",
    },
    SettingRow {
        label: "本文",
        kind: Kind::Multiline,
        slot: Slot::Event(|e| &mut e.message),
        help: "セル内の改行(Alt+Enter)がそのまま改行になります。「{時候}」は時候の挨拶に置き換わります",
    },
    SettingRow {
        label: "差出人の郵便番号",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.sender_postal),
        help: "封筒の裏面に入ります",
    },
    SettingRow {
        label: "差出人の住所",
        kind: Kind::Text,
        slot: Slot::Event(|e| &mut e.sender_address),
        help: "封筒の裏面に入ります",
    },
    SettingRow {
        label: "トンボ",
        kind: Kind::Flag,
        slot: Slot::CropMarks,
        help: "招待状カードにトンボを付けるか",
    },
    SettingRow {
        label: "封筒の裏面",
        kind: Kind::Flag,
        slot: Slot::EnvelopeBack,
        help: "差出人の裏面ページを付けるか",
    },
];

const FLAG_ON: &str = "する";
const FLAG_OFF: &str = "しない";

const GUEST_COLUMNS: [&str; 6] = ["氏名", "郵便番号", "住所1", "住所2", "連名", "敬称"];
const GUEST_HELP: [&str; 6] = [
    "姓と名の間にスペース",
    "123-4567",
    "都道府県から番地まで",
    "建物名・部屋番号",
    "名のみ、またはフルネーム",
    "空欄なら「様」",
];

fn guest_slot(g: &mut Guest, col: usize) -> &mut String {
    match col {
        0 => &mut g.name,
        1 => &mut g.postal_code,
        2 => &mut g.address1,
        3 => &mut g.address2,
        4 => &mut g.joint_name,
        _ => &mut g.honorific,
    }
}

/// ゲスト表の見出し(CSV と同じ別名も受け付ける).
fn guest_column(header: &str) -> Option<usize> {
    Some(match header.trim().to_lowercase().as_str() {
        "氏名" | "名前" | "お名前" | "name" => 0,
        "郵便番号" | "〒" | "postal_code" => 1,
        "住所1" | "住所" | "address1" => 2,
        "住所2" | "建物名" | "address2" => 3,
        "連名" | "joint_name" => 4,
        "敬称" | "honorific" => 5,
        _ => return None,
    })
}

// ------------------------------------------------------------------ 書き出し

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' | '\t' => out.push(ch),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

fn col_letter(col: usize) -> char {
    (b'A' + col as u8) as char
}

/// 文字列セル(インライン文字列).
fn cell(out: &mut String, col: usize, row: usize, style: u32, text: &str) {
    let _ = write!(
        out,
        r#"<c r="{}{row}" s="{style}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
        col_letter(col),
        esc(text)
    );
}

// スタイル番号(styles.xml の cellXfs の並び)
const S_TEXT: u32 = 1;
const S_HEADER: u32 = 2;
const S_WRAP: u32 = 3;
const S_HELP: u32 = 4;
const S_LABEL: u32 = 5;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="3"><font><sz val="11"/><name val="Yu Gothic"/><family val="3"/><charset val="128"/></font><font><b/><sz val="11"/><name val="Yu Gothic"/><family val="3"/><charset val="128"/></font><font><sz val="9"/><color rgb="FF7A726A"/><name val="Yu Gothic"/><family val="3"/><charset val="128"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFF3EBDD"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="6"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="49" fontId="1" fillId="2" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1" applyFill="1"/><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="49" fontId="2" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="49" fontId="1" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1" applyAlignment="1"><alignment vertical="top"/></xf></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#;

const SHEET_HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#;

const FROZEN_HEADER: &str = r#"<sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight="18"/>"#;

fn settings_value(order: &Order, row: &SettingRow) -> String {
    match row.slot {
        Slot::Design => designs::get(&order.design).name.to_string(),
        Slot::Event(f) => f(&mut order.event.clone()).clone(),
        Slot::CropMarks => (if order.options.crop_marks { FLAG_ON } else { FLAG_OFF }).into(),
        Slot::EnvelopeBack => (if order.options.envelope_back { FLAG_ON } else { FLAG_OFF }).into(),
    }
}

fn settings_sheet(order: &Order) -> String {
    let mut x = String::from(SHEET_HEAD);
    x.push_str(FROZEN_HEADER);
    x.push_str(&format!(
        r#"<cols><col min="1" max="1" width="20" style="{S_LABEL}" customWidth="1"/><col min="2" max="2" width="60" style="{S_WRAP}" customWidth="1"/><col min="3" max="3" width="48" style="{S_HELP}" customWidth="1"/></cols><sheetData>"#
    ));
    x.push_str(r#"<row r="1">"#);
    for (i, h) in ["項目", "値", "説明"].iter().enumerate() {
        cell(&mut x, i, 1, S_HEADER, h);
    }
    x.push_str("</row>");
    let mut validations = Vec::new();
    for (i, row) in SETTINGS.iter().enumerate() {
        let r = i + 2;
        let value = settings_value(order, row);
        let height = if row.kind == Kind::Multiline {
            let lines = value.lines().count().max(4) as f32;
            format!(r#" ht="{:.0}" customHeight="1""#, lines * 18.0 + 6.0)
        } else {
            String::new()
        };
        let _ = write!(x, r#"<row r="{r}"{height}>"#);
        cell(&mut x, 0, r, S_LABEL, row.label);
        cell(&mut x, 1, r, S_WRAP, &value);
        let help = match row.kind {
            Kind::Design => {
                let names: Vec<_> = designs::DESIGNS.iter().map(|d| d.name).collect();
                format!("{}: {}", row.help, names.join(" / "))
            }
            Kind::Flag => format!("{}(「{FLAG_ON}」か「{FLAG_OFF}」)", row.help),
            _ => row.help.to_string(),
        };
        cell(&mut x, 2, r, S_HELP, &help);
        x.push_str("</row>");
        let list = match row.kind {
            Kind::Design => Some(designs::DESIGNS.iter().map(|d| d.name).collect::<Vec<_>>().join(",")),
            Kind::Flag => Some(format!("{FLAG_ON},{FLAG_OFF}")),
            _ => None,
        };
        if let Some(list) = list {
            validations.push(format!(
                r#"<dataValidation type="list" allowBlank="1" showErrorMessage="1" sqref="B{r}"><formula1>"{}"</formula1></dataValidation>"#,
                esc(&list)
            ));
        }
    }
    x.push_str("</sheetData>");
    let _ = write!(
        x,
        r#"<dataValidations count="{}">{}</dataValidations>"#,
        validations.len(),
        validations.concat()
    );
    x.push_str(
        r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#,
    );
    x
}

fn guest_sheet(order: &Order) -> String {
    let mut x = String::from(SHEET_HEAD);
    x.push_str(FROZEN_HEADER);
    x.push_str("<cols>");
    for (i, w) in [16, 12, 36, 28, 12, 8].iter().enumerate() {
        let _ = write!(
            x,
            r#"<col min="{0}" max="{0}" width="{w}" style="{S_TEXT}" customWidth="1"/>"#,
            i + 1
        );
    }
    x.push_str("</cols><sheetData>");
    x.push_str(r#"<row r="1">"#);
    for (i, h) in GUEST_COLUMNS.iter().enumerate() {
        cell(&mut x, i, 1, S_HEADER, h);
    }
    x.push_str("</row>");
    for (i, g) in order.guests.iter().enumerate() {
        let r = i + 2;
        let _ = write!(x, r#"<row r="{r}">"#);
        let mut g = g.clone();
        for col in 0..GUEST_COLUMNS.len() {
            cell(&mut x, col, r, S_TEXT, guest_slot(&mut g, col));
        }
        x.push_str("</row>");
    }
    x.push_str("</sheetData>");
    // 見出しのコメント代わりに、記入例を入力規則のメッセージとして出す
    x.push_str(r#"<dataValidations count="6">"#);
    for (i, help) in GUEST_HELP.iter().enumerate() {
        let c = col_letter(i);
        let _ = write!(
            x,
            r#"<dataValidation allowBlank="1" showInputMessage="1" promptTitle="{}" prompt="{}" sqref="{c}2:{c}{}"/>"#,
            esc(GUEST_COLUMNS[i]),
            esc(help),
            MAX_GUESTS + 1
        );
    }
    x.push_str("</dataValidations>");
    x.push_str(
        r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#,
    );
    x
}

/// 入力内容を xlsx にする(そのまま入力用のテンプレートとして使える).
pub fn write_workbook(order: &Order) -> Vec<u8> {
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let workbook = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><bookViews><workbookView activeTab="0"/></bookViews><sheets><sheet name="{SETTINGS_SHEET}" sheetId="1" r:id="rId1"/><sheet name="{GUEST_SHEET}" sheetId="2" r:id="rId2"/></sheets></workbook>"#
    );
    let workbook_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let settings = settings_sheet(order);
    let guests = guest_sheet(order);
    zip::write(
        &[
            ("[Content_Types].xml", content_types.as_bytes()),
            ("_rels/.rels", root_rels.as_bytes()),
            ("xl/workbook.xml", workbook.as_bytes()),
            ("xl/_rels/workbook.xml.rels", workbook_rels.as_bytes()),
            ("xl/styles.xml", STYLES.as_bytes()),
            ("xl/worksheets/sheet1.xml", settings.as_bytes()),
            ("xl/worksheets/sheet2.xml", guests.as_bytes()),
        ],
        true,
    )
}

// ------------------------------------------------------------------ 読み込み

#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Text(String),
    Number(f64),
}

impl Cell {
    fn text(&self) -> String {
        match self {
            Cell::Text(s) => s.trim().to_string(),
            Cell::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            Cell::Number(n) => format!("{n}"),
        }
    }
}

type Grid = Vec<Vec<Option<Cell>>>;

fn xml_err(e: impl std::fmt::Display) -> String {
    format!("Excel ファイルの内容を読み取れません: {e}")
}

fn attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

/// sharedStrings.xml。ふりがな(rPh)は読まない.
fn parse_shared_strings(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let (mut cur, mut in_t, mut in_rph) = (String::new(), false, false);
    loop {
        match reader.read_event().map_err(xml_err)? {
            XmlEvent::Start(e) => match e.local_name().as_ref() {
                b"si" => cur.clear(),
                b"t" => in_t = true,
                b"rPh" => in_rph = true,
                _ => {}
            },
            XmlEvent::End(e) => match e.local_name().as_ref() {
                b"si" => out.push(std::mem::take(&mut cur)),
                b"t" => in_t = false,
                b"rPh" => in_rph = false,
                _ => {}
            },
            XmlEvent::Empty(e) if e.local_name().as_ref() == b"si" => out.push(String::new()),
            XmlEvent::Text(t) if in_t && !in_rph => cur.push_str(&t.unescape().map_err(xml_err)?),
            XmlEvent::CData(t) if in_t && !in_rph => cur.push_str(&String::from_utf8_lossy(&t)),
            XmlEvent::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

/// "B12" -> (行 11, 列 1)(0 始まり).
fn cell_ref(r: &str) -> Option<(usize, usize)> {
    let letters: String = r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let row: usize = r[letters.len()..].parse().ok()?;
    let col = letters
        .bytes()
        .fold(0usize, |acc, b| acc * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize);
    Some((row.checked_sub(1)?, col.checked_sub(1)?))
}

fn parse_sheet(xml: &str, shared: &[String]) -> Result<Grid, String> {
    let mut reader = Reader::from_str(xml);
    let mut grid: Grid = Vec::new();
    let (mut row, mut col) = (0usize, 0usize);
    let mut kind = String::new();
    let (mut value, mut inline) = (String::new(), String::new());
    let (mut in_v, mut in_t, mut in_rph, mut in_cell) = (false, false, false, false);
    let put = |grid: &mut Grid, row: usize, col: usize, cell: Option<Cell>| {
        if row > MAX_GUESTS + 100 || col > 50 {
            return;
        }
        if grid.len() <= row {
            grid.resize(row + 1, Vec::new());
        }
        if grid[row].len() <= col {
            grid[row].resize(col + 1, None);
        }
        grid[row][col] = cell;
    };
    loop {
        let ev = reader.read_event().map_err(xml_err)?;
        match &ev {
            XmlEvent::Start(e) | XmlEvent::Empty(e) => match e.local_name().as_ref() {
                b"row" => {
                    if let Some(r) = attr(e, b"r").and_then(|r| r.parse::<usize>().ok()) {
                        row = r.saturating_sub(1);
                    }
                    col = 0;
                }
                b"c" => {
                    if let Some((r, c)) = attr(e, b"r").as_deref().and_then(cell_ref) {
                        row = r;
                        col = c;
                    }
                    kind = attr(e, b"t").unwrap_or_default();
                    value.clear();
                    inline.clear();
                    in_cell = matches!(ev, XmlEvent::Start(_));
                    if !in_cell {
                        col += 1;
                    }
                }
                b"v" => in_v = true,
                b"t" => in_t = true,
                b"rPh" => in_rph = true,
                _ => {}
            },
            XmlEvent::End(e) => match e.local_name().as_ref() {
                b"v" => in_v = false,
                b"t" => in_t = false,
                b"rPh" => in_rph = false,
                b"row" => {
                    row += 1;
                }
                b"c" if in_cell => {
                    let cell = match kind.as_str() {
                        "s" => value
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|i| shared.get(i))
                            .map(|s| Cell::Text(s.clone())),
                        "inlineStr" => Some(Cell::Text(inline.clone())),
                        "b" => Some(Cell::Text(if value.trim() == "1" { "TRUE" } else { "FALSE" }.into())),
                        "str" | "e" => Some(Cell::Text(value.clone())),
                        _ if value.trim().is_empty() => None,
                        _ => value.trim().parse::<f64>().ok().map(Cell::Number),
                    };
                    put(&mut grid, row, col, cell);
                    in_cell = false;
                    col += 1;
                }
                _ => {}
            },
            XmlEvent::Text(t) => {
                let s = t.unescape().map_err(xml_err)?;
                if in_v {
                    value.push_str(&s);
                } else if in_t && !in_rph {
                    inline.push_str(&s);
                }
            }
            XmlEvent::Eof => break,
            _ => {}
        }
    }
    Ok(grid)
}

/// ブック内のシート名 -> シートの XML のパス.
fn sheet_paths(files: &HashMap<String, Vec<u8>>) -> Result<Vec<(String, String)>, String> {
    let get = |name: &str| -> Result<String, String> {
        let data = files
            .get(name)
            .ok_or_else(|| "Excel ファイル(xlsx)ではないようです".to_string())?;
        Ok(String::from_utf8_lossy(data).into_owned())
    };
    let rels_xml = get("xl/_rels/workbook.xml.rels")?;
    let mut rels = HashMap::new();
    let mut reader = Reader::from_str(&rels_xml);
    loop {
        match reader.read_event().map_err(xml_err)? {
            XmlEvent::Start(e) | XmlEvent::Empty(e) if e.local_name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, b"Id"), attr(&e, b"Target")) {
                    let path = match target.strip_prefix('/') {
                        Some(abs) => abs.to_string(),
                        None => format!("xl/{target}"),
                    };
                    rels.insert(id, path);
                }
            }
            XmlEvent::Eof => break,
            _ => {}
        }
    }
    let workbook = get("xl/workbook.xml")?;
    let mut sheets = Vec::new();
    let mut reader = Reader::from_str(&workbook);
    loop {
        match reader.read_event().map_err(xml_err)? {
            XmlEvent::Start(e) | XmlEvent::Empty(e) if e.local_name().as_ref() == b"sheet" => {
                if let (Some(name), Some(id)) = (attr(&e, b"name"), attr(&e, b"id")) {
                    if let Some(path) = rels.get(&id) {
                        sheets.push((name, path.clone()));
                    }
                }
            }
            XmlEvent::Eof => break,
            _ => {}
        }
    }
    Ok(sheets)
}

/// 文字列の中の数字のまとまりを取り出す(全角数字も可).
fn numbers(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cur: Option<u32> = None;
    for ch in s.chars() {
        let d = match ch {
            '0'..='9' => Some(ch as u32 - '0' as u32),
            '０'..='９' => Some(ch as u32 - '０' as u32),
            _ => None,
        };
        match (d, cur) {
            (Some(d), Some(c)) => cur = Some(c.saturating_mul(10).saturating_add(d)),
            (Some(d), None) => cur = Some(d),
            (None, Some(c)) => {
                out.push(c);
                cur = None;
            }
            (None, None) => {}
        }
    }
    out.extend(cur);
    out
}

/// Excel のシリアル値(1900 年方式)の日付部分.
fn excel_date(serial: f64) -> Date {
    Date::from_unix_days(serial.floor() as i64 - 25569)
}

fn to_date(cell: &Cell) -> Result<String, String> {
    match cell {
        Cell::Number(n) if *n > 0.0 => Ok(excel_date(*n).iso()),
        _ => {
            let text = cell.text();
            let n = numbers(&text);
            if n.len() >= 3 && n[0] >= 1900 {
                let iso = format!("{:04}-{:02}-{:02}", n[0], n[1], n[2]);
                if Date::parse(&iso).is_some() {
                    return Ok(iso);
                }
            }
            Err(text)
        }
    }
}

fn to_time(cell: &Cell) -> Result<String, String> {
    match cell {
        Cell::Number(n) if *n >= 0.0 => {
            let minutes = (n.fract() * 1440.0).round() as u32 % 1440;
            Ok(format!("{:02}:{:02}", minutes / 60, minutes % 60))
        }
        _ => {
            let text = cell.text();
            let n = numbers(&text);
            let Some(&h) = n.first() else { return Err(text) };
            let h = if text.contains("午後") && h < 12 { h + 12 } else { h };
            let m = n.get(1).copied().unwrap_or(0);
            if h < 24 && m < 60 {
                Ok(format!("{h:02}:{m:02}"))
            } else {
                Err(text)
            }
        }
    }
}

fn to_flag(text: &str) -> Option<bool> {
    match text.trim().to_lowercase().as_str() {
        "する" | "付ける" | "つける" | "あり" | "有" | "はい" | "○" | "〇" | "yes" | "true" | "1" | "on" => {
            Some(true)
        }
        "しない" | "付けない" | "つけない" | "なし" | "無" | "いいえ" | "×" | "no" | "false" | "0" | "off" => {
            Some(false)
        }
        _ => None,
    }
}

fn find_design(text: &str) -> Option<&'static str> {
    let t = text.trim();
    designs::DESIGNS
        .iter()
        .find(|d| d.name == t || d.key.eq_ignore_ascii_case(t))
        .or_else(|| {
            designs::DESIGNS
                .iter()
                .find(|d| !t.is_empty() && (d.name.contains(t) || t.contains(d.name)))
        })
        .map(|d| d.key)
}

/// 郵便番号が数値として保存され先頭の 0 が消えた場合に戻す(例: 600001 -> 0600001).
fn fix_postal(cell: &Cell) -> String {
    match cell {
        Cell::Number(n) if n.fract() == 0.0 && *n >= 0.0 && *n < 1e7 => format!("{:07}", *n as u64),
        _ => cell.text(),
    }
}

#[derive(Debug, Serialize)]
pub struct Imported {
    pub order: Order,
    /// 読み込めなかった項目などの注意
    pub warnings: Vec<String>,
}

/// xlsx を読み込み、画面の入力内容を返す.
pub fn read_workbook(data: &[u8]) -> Result<Imported, String> {
    let files: HashMap<String, Vec<u8>> = zip::read(data)?.into_iter().collect();
    let shared = match files.get("xl/sharedStrings.xml") {
        Some(d) => parse_shared_strings(&String::from_utf8_lossy(d))?,
        None => Vec::new(),
    };
    let sheets = sheet_paths(&files)?;
    let grid_of = |path: &str| -> Result<Grid, String> {
        let xml = files
            .get(path)
            .ok_or_else(|| format!("シート {path} が見つかりません"))?;
        parse_sheet(&String::from_utf8_lossy(xml), &shared)
    };
    let find = |name: &str, fallback: usize| sheets.iter().find(|(n, _)| n == name).or_else(|| sheets.get(fallback));

    let mut order = Order::default();
    let mut warnings = Vec::new();
    let mut seen_settings = false;

    if let Some((_, path)) = find(SETTINGS_SHEET, 0) {
        for row in grid_of(path)? {
            let Some(label) = row.first().and_then(|c| c.as_ref()).map(Cell::text) else {
                continue;
            };
            let Some(setting) = SETTINGS.iter().find(|s| s.label == label) else {
                continue;
            };
            seen_settings = true;
            let cell = row.get(1).cloned().flatten();
            let text = cell.as_ref().map(Cell::text).unwrap_or_default();
            match setting.slot {
                Slot::Design => match find_design(&text) {
                    Some(key) => order.design = key.into(),
                    None => warnings.push(format!(
                        "デザイン「{text}」が見つからないため「{}」にしました",
                        designs::DESIGNS[0].name
                    )),
                },
                Slot::CropMarks | Slot::EnvelopeBack => {
                    let value = if text.is_empty() { Some(true) } else { to_flag(&text) };
                    let value = value.unwrap_or_else(|| {
                        warnings.push(format!(
                            "「{}」の値「{text}」は「{FLAG_ON}」か「{FLAG_OFF}」で入力してください",
                            setting.label
                        ));
                        true
                    });
                    match setting.slot {
                        Slot::CropMarks => order.options.crop_marks = value,
                        _ => order.options.envelope_back = value,
                    }
                }
                Slot::Event(f) => {
                    let value = match (&cell, setting.kind) {
                        (None, _) => String::new(),
                        (Some(c), Kind::Date) => to_date(c).unwrap_or_else(|t| {
                            warnings.push(format!("「{}」の日付「{t}」を読み取れません", setting.label));
                            String::new()
                        }),
                        (Some(c), Kind::Time) => to_time(c).unwrap_or_else(|t| {
                            warnings.push(format!("「{}」の時刻「{t}」を読み取れません", setting.label));
                            String::new()
                        }),
                        (Some(c), Kind::Multiline) => match c {
                            Cell::Text(s) => s.replace("\r\n", "\n").trim_matches('\n').to_string(),
                            other => other.text(),
                        },
                        (Some(c), _) => c.text(),
                    };
                    *f(&mut order.event) = value;
                }
            }
        }
    }
    if !seen_settings {
        warnings.push(format!(
            "「{SETTINGS_SHEET}」シートが見つからないため、挙式情報は初期値のままです"
        ));
        order.event = Event::default();
    }
    order.event = order.event.cleaned();

    if let Some((_, path)) = find(GUEST_SHEET, 1) {
        let grid = grid_of(path)?;
        let header_row = grid
            .iter()
            .position(|row| row.iter().flatten().any(|c| guest_column(&c.text()) == Some(0)));
        match header_row {
            Some(h) => {
                let columns: Vec<Option<usize>> = grid[h]
                    .iter()
                    .map(|c| c.as_ref().and_then(|c| guest_column(&c.text())))
                    .collect();
                for row in &grid[h + 1..] {
                    let mut g = Guest::default();
                    for (cell, col) in row.iter().zip(&columns) {
                        let (Some(cell), Some(col)) = (cell, col) else { continue };
                        *guest_slot(&mut g, *col) = if *col == 1 { fix_postal(cell) } else { cell.text() };
                    }
                    let g = g.cleaned();
                    if !g.name.is_empty() {
                        order.guests.push(g);
                    }
                }
            }
            None => warnings.push(format!("「{GUEST_SHEET}」シートに「氏名」の見出しが見つかりません")),
        }
    } else {
        warnings.push(format!("「{GUEST_SHEET}」シートが見つかりません"));
    }
    if order.guests.len() > MAX_GUESTS {
        warnings.push(format!(
            "ゲストは {MAX_GUESTS} 名までのため、それ以降は読み込んでいません"
        ));
        order.guests.truncate(MAX_GUESTS);
    }
    Ok(Imported { order, warnings })
}

/// 画面の初期状態(サンプル入り)のテンプレート.
pub fn template() -> Vec<u8> {
    let order = Order {
        guests: crate::csv::sample_guests(),
        options: Options::default(),
        ..Order::default()
    };
    write_workbook(&order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_order() -> Order {
        Order {
            design: "yuki".into(),
            event: Event {
                date_line: "2027年1月吉日".into(),
                ..Event::default()
            },
            guests: vec![
                Guest::new(
                    "山田 太郎",
                    "0600001",
                    "北海道札幌市中央区北1条西2-3",
                    "A&B <ビル> \"5F\"",
                    "花子",
                ),
                Guest::new("鈴木 一子", "530-0001", "大阪府大阪市北区梅田3-1-1", "", ""),
            ],
            options: Options {
                crop_marks: false,
                envelope_back: true,
            },
        }
    }

    #[test]
    fn round_trip_keeps_everything() {
        let order = sample_order();
        let back = read_workbook(&write_workbook(&order)).unwrap();
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        assert_eq!(back.order, order);
        assert!(back.order.event.message.contains('\n'));
    }

    #[test]
    fn template_has_sample_guests() {
        let back = read_workbook(&template()).unwrap();
        assert_eq!(back.order.guests.len(), 2);
        assert_eq!(back.order.design, designs::DEFAULT_DESIGN);
    }

    #[test]
    fn cell_values_from_excel() {
        // 2027-04-10 のシリアル値は 46487
        assert_eq!(to_date(&Cell::Number(46487.0)).unwrap(), "2027-04-10");
        assert_eq!(to_date(&Cell::Text("2027/4/10".into())).unwrap(), "2027-04-10");
        assert_eq!(
            to_date(&Cell::Text("２０２７年４月１０日（土）".into())).unwrap(),
            "2027-04-10"
        );
        assert!(to_date(&Cell::Text("来年の春".into())).is_err());
        assert_eq!(to_time(&Cell::Number(0.4375)).unwrap(), "10:30");
        assert_eq!(to_time(&Cell::Text("午後1時30分".into())).unwrap(), "13:30");
        assert_eq!(to_time(&Cell::Text("11:00".into())).unwrap(), "11:00");
        assert_eq!(fix_postal(&Cell::Number(600001.0)), "0600001");
        assert_eq!(find_design("睦月"), Some("mutsuki"));
        assert_eq!(find_design("modern"), Some("modern"));
        assert_eq!(to_flag("付けない"), Some(false));
    }

    #[test]
    fn shared_strings_skip_furigana() {
        let xml = r#"<sst><si><t>山田</t><rPh sb="0" eb="2"><t>ヤマダ</t></rPh></si><si><r><t>太</t></r><r><t>郎</t></r></si></sst>"#;
        assert_eq!(parse_shared_strings(xml).unwrap(), vec!["山田", "太郎"]);
    }

    #[test]
    fn sheet_cells() {
        let shared = vec!["氏名".to_string(), "山田 太郎".to_string()];
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" t="inlineStr"><is><t>住所1</t></is></c></row><row r="3"><c t="s"><v>1</v></c><c><v>600001</v></c><c t="b"><v>1</v></c></row></sheetData></worksheet>"#;
        let grid = parse_sheet(xml, &shared).unwrap();
        assert_eq!(grid[0][0], Some(Cell::Text("氏名".into())));
        assert_eq!(grid[0][1], None);
        assert_eq!(grid[0][2], Some(Cell::Text("住所1".into())));
        assert_eq!(grid[2][0], Some(Cell::Text("山田 太郎".into())));
        assert_eq!(grid[2][1], Some(Cell::Number(600001.0)));
        assert_eq!(grid[2][2], Some(Cell::Text("TRUE".into())));
    }

    #[test]
    fn not_xlsx() {
        assert!(read_workbook(b"hello").is_err());
        let zip = zip::write(&[("a.txt", b"x")], true);
        assert!(read_workbook(&zip).is_err());
    }
}
