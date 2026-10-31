//! 入力内容を xlsx(Excel)に書き出し、読み込む.
//!
//! 「設定」シートにデザイン・挙式情報・出力オプションを、「出席者名簿」シートに名簿を
//! One-W と同じ列で置く。One-W から書き出した名簿だけの xlsx もそのまま読み込める。
//! 読み込みは見出し(項目名)で対応を取るので、行や列の順番が入れ替わっても読める。
//! Excel が日付・時刻・数値として保存したセルも文字列に直して取り込む。

use std::collections::HashMap;
use std::fmt::Write;

use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use serde::Serialize;

use crate::designs;
use crate::model::{
    Access, AccessRoute, Date, Event, Guest, Options, Order, RouteStep, ACCESS_MODES, CARD_LAYOUTS, LANDSCAPE,
    MAX_GUESTS, MAX_ROUTES, MAX_STEPS, PORTRAIT,
};
use crate::roster::{self, ONEW_SHEET};
use crate::zip;

pub const SETTINGS_SHEET: &str = "設定";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Date,
    Time,
    Multiline,
    /// する/しない
    Flag,
    /// 縦/横
    Orientation,
    /// 横書き/縦書き/写真入り
    CardLayout,
    Design,
    /// 招待状と同じ / デザイン名
    AccessDesign,
}

#[derive(Clone, Copy)]
enum Slot {
    Design,
    Event(fn(&mut Event) -> &mut String),
    Access(fn(&mut Access) -> &mut String),
    CropMarks,
    EnvelopeBack,
    AccessOrientation,
    AccessDesign,
    CardLayout,
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
        label: "招待状のレイアウト",
        kind: Kind::CardLayout,
        slot: Slot::CardLayout,
        help: "本文の組み方。写真入りの写真は画面で選びます",
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
    SettingRow {
        label: "案内図のデザイン",
        kind: Kind::AccessDesign,
        slot: Slot::AccessDesign,
        help: "案内図だけ別のデザインにするとき",
    },
    SettingRow {
        label: "案内図の向き",
        kind: Kind::Orientation,
        slot: Slot::AccessOrientation,
        help: "ハガキを縦(100×148mm)にするか横(148×100mm)にするか",
    },
    SettingRow {
        label: "案内図のタイトル",
        kind: Kind::Text,
        slot: Slot::Access(|a| &mut a.title),
        help: "案内図(ハガキ)の見出し。交通手段は「アクセス」シートに書きます(写真は画面で追加)",
    },
    SettingRow {
        label: "道順の見出し",
        kind: Kind::Text,
        slot: Slot::Access(|a| &mut a.steps_title),
        help: "道順写真の見出し。空なら「最寄り駅からの道順」(例: 南海なんば駅からの道順)",
    },
    SettingRow {
        label: "お車の道順の見出し",
        kind: Kind::Text,
        slot: Slot::Access(|a| &mut a.car_steps_title),
        help: "お車の道順写真(別ページ)の見出し。空なら「お車でご来場の方」。写真は画面で追加",
    },
    SettingRow {
        label: "お車の案内",
        kind: Kind::Multiline,
        slot: Slot::Access(|a| &mut a.car_note),
        help: "お車の道順のページの下に枠で添える文(例: 駐車場の案内)",
    },
    SettingRow {
        label: "館内図の見出し",
        kind: Kind::Text,
        slot: Slot::Access(|a| &mut a.floor_map_title),
        help: "館内図(別ページ)の見出し。空なら「館内図」。画像は画面で追加",
    },
    SettingRow {
        label: "館内図の案内",
        kind: Kind::Multiline,
        slot: Slot::Access(|a| &mut a.floor_note),
        help: "館内図のページの下に枠で添える文(例: 工事のお知らせ)",
    },
    SettingRow {
        label: "案内図の補足",
        kind: Kind::Multiline,
        slot: Slot::Access(|a| &mut a.note),
        help: "例: 送迎バスの時刻",
    },
];

pub const ACCESS_SHEET: &str = "アクセス";
pub const STEPS_SHEET: &str = "道順";
/// 「道順」シートの道順の種類(駅から / お車)
const STEP_GROUPS: [&str; 2] = ["駅から", "お車"];

const FLAG_ON: &str = "する";
const FLAG_OFF: &str = "しない";

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

/// 0 始まりの列番号 -> "A", "B", …, "Z", "AA", ….
fn col_letter(col: usize) -> String {
    let mut n = col + 1;
    let mut out = Vec::new();
    while n > 0 {
        out.push(b'A' + ((n - 1) % 26) as u8);
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
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

/// 「案内図のデザイン」で招待状と同じにする値
const SAME_AS_CARD: &str = "招待状と同じ";

fn settings_value(order: &Order, row: &SettingRow) -> String {
    match row.slot {
        Slot::Design => designs::get(&order.design).name.to_string(),
        Slot::Event(f) => f(&mut order.event.clone()).clone(),
        Slot::Access(f) => f(&mut order.access.clone()).clone(),
        Slot::CropMarks => (if order.options.crop_marks { FLAG_ON } else { FLAG_OFF }).into(),
        Slot::EnvelopeBack => (if order.options.envelope_back { FLAG_ON } else { FLAG_OFF }).into(),
        Slot::AccessOrientation => (if order.access.is_landscape() { "横" } else { "縦" }).into(),
        Slot::AccessDesign => match designs::find(&order.access.design) {
            Some(d) => d.name.into(),
            None => SAME_AS_CARD.into(),
        },
        Slot::CardLayout => order.card.label().into(),
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
            Kind::AccessDesign => {
                let names: Vec<_> = designs::DESIGNS.iter().map(|d| d.name).collect();
                format!("{}: {SAME_AS_CARD} / {}", row.help, names.join(" / "))
            }
            Kind::Flag => format!("{}(「{FLAG_ON}」か「{FLAG_OFF}」)", row.help),
            Kind::Orientation => format!("{}(「縦」か「横」)", row.help),
            Kind::CardLayout => format!(
                "{}: {}",
                row.help,
                CARD_LAYOUTS.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(" / ")
            ),
            _ => row.help.to_string(),
        };
        cell(&mut x, 2, r, S_HELP, &help);
        x.push_str("</row>");
        let list = match row.kind {
            Kind::Design => Some(designs::DESIGNS.iter().map(|d| d.name).collect::<Vec<_>>().join(",")),
            Kind::AccessDesign => Some(
                std::iter::once(SAME_AS_CARD)
                    .chain(designs::DESIGNS.iter().map(|d| d.name))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            Kind::Flag => Some(format!("{FLAG_ON},{FLAG_OFF}")),
            Kind::Orientation => Some("縦,横".to_string()),
            Kind::CardLayout => Some(CARD_LAYOUTS.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(",")),
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

/// 1 行目を見出しにした表のシート(ゲスト名簿用).
fn table_sheet(rows: &[Vec<String>]) -> String {
    let mut x = String::from(SHEET_HEAD);
    x.push_str(FROZEN_HEADER);
    x.push_str("<cols>");
    let width = |h: &str| match h {
        "説明" => 64,
        "表示名" => 14,
        "種類" => 12,
        "住所1" => 30,
        "住所2" => 30,
        "備考" | "特記事項" | "配席メモ" => 24,
        "PC Email" | "携帯 Email" => 22,
        "肩書き" | "姓ふりがな" | "名ふりがな" | "外字ファイル名" | "携帯電話番号" | "電話番号" => {
            14
        }
        _ => 10,
    };
    for (i, h) in rows.first().into_iter().flatten().enumerate() {
        let _ = write!(
            x,
            r#"<col min="{0}" max="{0}" width="{1}" style="{S_TEXT}" customWidth="1"/>"#,
            i + 1,
            width(h)
        );
    }
    x.push_str("</cols><sheetData>");
    for (i, row) in rows.iter().enumerate() {
        let r = i + 1;
        let style = if i == 0 { S_HEADER } else { S_TEXT };
        let _ = write!(x, r#"<row r="{r}">"#);
        for (col, value) in row.iter().enumerate() {
            if !value.is_empty() || i == 0 {
                cell(&mut x, col, r, style, value);
            }
        }
        x.push_str("</row>");
    }
    x.push_str("</sheetData>");
    x.push_str(
        r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#,
    );
    x
}

/// シートの並びから xlsx を組み立てる.
fn build_workbook(sheets: &[(&str, String)]) -> Vec<u8> {
    let mut content_types = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    let mut sheet_list = String::new();
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (i, (name, _)) in sheets.iter().enumerate() {
        let n = i + 1;
        let _ = write!(
            content_types,
            r#"<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        );
        let _ = write!(
            sheet_list,
            r#"<sheet name="{}" sheetId="{n}" r:id="rId{n}"/>"#,
            esc(name)
        );
        let _ = write!(
            rels,
            r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#
        );
    }
    content_types.push_str("</Types>");
    let _ = write!(
        rels,
        r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
        sheets.len() + 1
    );
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let workbook = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><bookViews><workbookView activeTab="0"/></bookViews><sheets>{sheet_list}</sheets></workbook>"#
    );
    let names: Vec<String> = (1..=sheets.len())
        .map(|n| format!("xl/worksheets/sheet{n}.xml"))
        .collect();
    let mut files: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", root_rels.as_bytes()),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
        ("xl/styles.xml", STYLES.as_bytes()),
    ];
    for (path, (_, xml)) in names.iter().zip(sheets) {
        files.push((path, xml.as_bytes()));
    }
    zip::write(&files, true)
}

/// 入力内容を xlsx にする(そのまま入力用のテンプレートとして使える).
///
/// ゲストは One-W の「出席者名簿」と同じ列で書き出す。
pub fn write_workbook(order: &Order) -> Vec<u8> {
    build_workbook(&[
        (SETTINGS_SHEET, settings_sheet(order)),
        (ONEW_SHEET, table_sheet(&roster::onew_table(&order.guests))),
        (ACCESS_SHEET, access_sheet(&order.access)),
        (STEPS_SHEET, steps_sheet(&order.access)),
    ])
}

/// 案内図の交通手段(種類はプルダウン).
fn access_sheet(access: &Access) -> String {
    let mut rows = vec![vec!["種類".to_string(), "説明".to_string(), "表示名".to_string()]];
    for r in &access.routes {
        let label = ACCESS_MODES
            .iter()
            .find(|(k, _)| *k == r.mode)
            .map(|(_, l)| *l)
            .unwrap_or("");
        rows.push(vec![label.to_string(), r.text.clone(), r.label.clone()]);
    }
    let labels: Vec<&str> = ACCESS_MODES.iter().map(|(_, l)| *l).collect();
    let validation = format!(
        r#"<dataValidations count="1"><dataValidation type="list" allowBlank="1" showErrorMessage="1" sqref="A2:A{}"><formula1>"{}"</formula1></dataValidation></dataValidations><pageMargins"#,
        MAX_ROUTES + 1,
        esc(&labels.join(","))
    );
    table_sheet(&rows).replacen("<pageMargins", &validation, 1)
}

/// 道順写真の説明(写真そのものは画面で追加する).
fn steps_sheet(access: &Access) -> String {
    let mut rows = vec![vec!["道順".to_string(), "番号".to_string(), "説明".to_string()]];
    for (group, steps) in STEP_GROUPS.iter().zip([&access.steps, &access.car_steps]) {
        for (i, st) in steps.iter().enumerate() {
            rows.push(vec![group.to_string(), (i + 1).to_string(), st.caption.clone()]);
        }
    }
    let validation = format!(
        r#"<dataValidations count="1"><dataValidation type="list" allowBlank="1" showErrorMessage="1" sqref="A2:A{}"><formula1>"{}"</formula1></dataValidation></dataValidations><pageMargins"#,
        MAX_STEPS * 2 + 1,
        esc(&STEP_GROUPS.join(","))
    );
    table_sheet(&rows).replacen("<pageMargins", &validation, 1)
}

/// 「道順」シートから説明を読む(駅から, お車)。写真は空のまま.
fn read_steps(rows: &[Vec<String>]) -> Option<(Vec<RouteStep>, Vec<RouteStep>)> {
    let h = rows.iter().position(|r| r.iter().any(|c| c.trim() == "道順"))?;
    let col = |name: &str| rows[h].iter().position(|c| c.trim() == name);
    let (group_col, text_col) = (col("道順")?, col("説明")?);
    let (mut station, mut car) = (Vec::new(), Vec::new());
    for row in &rows[h + 1..] {
        let get = |i: usize| row.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        let (group, caption) = (get(group_col), get(text_col));
        if caption.is_empty() {
            continue;
        }
        let step = RouteStep {
            photo: String::new(),
            caption,
        };
        if group.contains("車") {
            car.push(step);
        } else {
            station.push(step);
        }
    }
    Some((station, car))
}

/// 「アクセス」シートの表から交通手段を読む.
fn read_routes(rows: &[Vec<String>], warnings: &mut Vec<String>) -> Option<Vec<AccessRoute>> {
    let h = rows.iter().position(|r| r.iter().any(|c| c.trim() == "種類"))?;
    let col = |name: &str| rows[h].iter().position(|c| c.trim() == name);
    let (mode_col, text_col) = (col("種類")?, col("説明")?);
    // 「表示名」は後から足した列なので、なくてもよい
    let label_col = col("表示名");
    let mut routes = Vec::new();
    for row in &rows[h + 1..] {
        let get = |i: usize| row.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        let (label, text) = (get(mode_col), get(text_col));
        if text.is_empty() {
            continue;
        }
        let mode = ACCESS_MODES
            .iter()
            .find(|(k, l)| *l == label || k.eq_ignore_ascii_case(&label))
            .map(|(k, _)| *k);
        let mode = mode.unwrap_or_else(|| {
            warnings.push(format!("交通手段「{label}」が分からないため「電車」にしました"));
            "train"
        });
        routes.push(AccessRoute {
            mode: mode.into(),
            text,
            label: label_col.map(get).unwrap_or_default(),
        });
    }
    Some(routes)
}

/// ゲスト名簿だけを One-W の出席者名簿形式の xlsx にする.
pub fn write_guest_workbook(guests: &[Guest]) -> Vec<u8> {
    build_workbook(&[(ONEW_SHEET, table_sheet(&roster::onew_table(guests)))])
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

#[derive(Debug, Serialize)]
pub struct Imported {
    pub order: Order,
    /// 「設定」シートがあったか(なければ名簿だけのファイルとして扱う)
    pub has_settings: bool,
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

    let mut order = Order::default();
    let mut warnings = Vec::new();
    let mut seen_settings = false;

    if let Some((_, path)) = sheets.iter().find(|(n, _)| n == SETTINGS_SHEET) {
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
                Slot::AccessDesign => {
                    let t = text.trim();
                    order.access.design = if t.is_empty() || t == SAME_AS_CARD {
                        String::new()
                    } else if let Some(key) = find_design(t) {
                        key.into()
                    } else {
                        warnings.push(format!(
                            "「{}」の値「{text}」が見つからないため「{SAME_AS_CARD}」にしました",
                            setting.label
                        ));
                        String::new()
                    };
                }
                Slot::CardLayout => {
                    let found = CARD_LAYOUTS.iter().find(|(k, l)| {
                        *l == text.trim()
                            || k.eq_ignore_ascii_case(text.trim())
                            || (!text.trim().is_empty() && l.contains(text.trim()))
                    });
                    match found {
                        Some((k, _)) => order.card.layout = k.to_string(),
                        None if text.trim().is_empty() => {}
                        None => warnings.push(format!(
                            "「{}」の値「{text}」が分からないため「{}」にしました",
                            setting.label, CARD_LAYOUTS[0].1
                        )),
                    }
                }
                Slot::AccessOrientation => {
                    order.access.orientation = match text.trim() {
                        "横" | "横向き" | "landscape" => LANDSCAPE.into(),
                        "" | "縦" | "縦向き" | "portrait" => PORTRAIT.into(),
                        other => {
                            warnings.push(format!(
                                "「{}」の値「{other}」は「縦」か「横」で入力してください",
                                setting.label
                            ));
                            PORTRAIT.into()
                        }
                    };
                }
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
                Slot::Event(_) | Slot::Access(_) => {
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
                    match setting.slot {
                        Slot::Event(f) => *f(&mut order.event) = value,
                        Slot::Access(f) => *f(&mut order.access) = value,
                        _ => {}
                    }
                }
            }
        }
    }
    if !seen_settings {
        order.event = Event::default();
    }
    order.event = order.event.cleaned();

    if let Some((_, path)) = sheets.iter().find(|(n, _)| n == ACCESS_SHEET) {
        let rows: Vec<Vec<String>> = grid_of(path)?
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|c| c.map(|c| c.text()).unwrap_or_default())
                    .collect()
            })
            .collect();
        match read_routes(&rows, &mut warnings) {
            Some(routes) => order.access.routes = routes,
            None => warnings.push(format!(
                "「{ACCESS_SHEET}」シートに「種類」「説明」の見出しが見つかりません"
            )),
        }
    }
    if let Some((_, path)) = sheets.iter().find(|(n, _)| n == STEPS_SHEET) {
        let rows: Vec<Vec<String>> = grid_of(path)?
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|c| c.map(|c| c.text()).unwrap_or_default())
                    .collect()
            })
            .collect();
        match read_steps(&rows) {
            Some((station, car)) => {
                order.access.steps = station;
                order.access.car_steps = car;
            }
            None => warnings.push(format!(
                "「{STEPS_SHEET}」シートに「道順」「説明」の見出しが見つかりません"
            )),
        }
    }
    order.access = order.access.cleaned();

    // ゲスト名簿: 「出席者名簿」(旧「ゲスト」)を優先し、なければ見出しのあるシートを探す
    let mut ordered: Vec<&(String, String)> = sheets.iter().collect();
    ordered.sort_by_key(|(name, _)| match name.as_str() {
        ONEW_SHEET | "ゲスト" => 0,
        SETTINGS_SHEET | ACCESS_SHEET | STEPS_SHEET => 2,
        _ => 1,
    });
    let mut found = false;
    for (_, path) in ordered {
        let rows: Vec<Vec<String>> = grid_of(path)?
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|c| c.map(|c| c.text()).unwrap_or_default())
                    .collect()
            })
            .collect();
        if let Some(guests) = roster::guests_from_table(&rows) {
            order.guests = guests;
            found = true;
            break;
        }
    }
    if !found {
        warnings.push(format!(
            "ゲスト名簿のシート(「{ONEW_SHEET}」など、1 行目に「姓」「名」または「氏名」の見出しがあるもの)が見つかりません"
        ));
    }
    if order.guests.len() > MAX_GUESTS {
        warnings.push(format!(
            "ゲストは {MAX_GUESTS} 名までのため、それ以降は読み込んでいません"
        ));
        order.guests.truncate(MAX_GUESTS);
    }
    Ok(Imported {
        order,
        has_settings: seen_settings,
        warnings,
    })
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
            card: crate::model::CardStyle {
                layout: "vertical".into(),
                photo: String::new(),
            },
            access: crate::model::Access {
                note: "送迎バスあり\n10:00 発".into(),
                orientation: LANDSCAPE.into(),
                design: "sakura".into(),
                steps_title: "南海なんば駅からの道順".into(),
                car_steps_title: "お車の方".into(),
                steps: vec![
                    RouteStep {
                        photo: "p1".into(),
                        caption: "北改札を出て赤丸のホテルロゴへ".into(),
                    },
                    RouteStep {
                        photo: "p2".into(),
                        caption: "矢印先のエレベーターで 6F ロビーへ".into(),
                    },
                ],
                car_steps: vec![RouteStep {
                    photo: "p3".into(),
                    caption: "6F ロビー直結の車寄せへ".into(),
                }],
                ..crate::model::Access::default()
            },
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

    /// 写真(Excel に入らない)とゲスト(別の形式で保存される)を除いた入力内容.
    fn settings_only(o: &Order) -> Order {
        let mut o = o.clone();
        o.guests.clear();
        o.card.photo.clear();
        o.access.map_photo.clear();
        o.access.floor_map.clear();
        for st in o.access.steps.iter_mut().chain(o.access.car_steps.iter_mut()) {
            st.photo.clear();
        }
        o
    }

    /// 画面で選べる項目は、どれも既定値から変えて Excel を往復しても残る(画面と Excel が同期している).
    #[test]
    fn every_setting_survives_excel() {
        let step = |photo: &str, caption: &str| RouteStep {
            photo: photo.into(),
            caption: caption.into(),
        };
        let order = Order {
            design: "white".into(),
            event: Event {
                groom_name: "山田 一郎".into(),
                bride_name: "佐藤 花子".into(),
                date: "2027-05-15".into(),
                reception_open_time: "10:15".into(),
                ceremony_time: "11:00".into(),
                party_time: "12:30".into(),
                venue_name: "スイスホテル南海大阪".into(),
                venue_address: "大阪府大阪市中央区難波5-1-60".into(),
                venue_tel: "06-0000-0000".into(),
                rsvp_deadline: "2027-04-01".into(),
                message: "拝啓\n{時候}".into(),
                date_line: "2027年3月吉日".into(),
                sender_postal: "542-0076".into(),
                sender_address: "大阪府大阪市中央区".into(),
            },
            guests: Vec::new(),
            options: Options {
                crop_marks: false,
                envelope_back: false,
            },
            access: Access {
                title: "交通のご案内".into(),
                map_photo: "map".into(),
                routes: vec![
                    AccessRoute {
                        mode: "train".into(),
                        text: "南海なんば駅 3F 北改札口からホテル直結".into(),
                        label: "南海電鉄".into(),
                    },
                    AccessRoute {
                        mode: "subway".into(),
                        text: "御堂筋線 なんば駅から地下通路で徒歩約 4 分".into(),
                        ..AccessRoute::default()
                    },
                    AccessRoute {
                        mode: "taxi".into(),
                        text: "ホテルロビー 6F にお越しください".into(),
                        ..AccessRoute::default()
                    },
                ],
                steps: vec![step("s1", "北改札を出て赤丸のホテルロゴへ"), step("s2", "")],
                note: "当日は 9:45 までに".into(),
                orientation: LANDSCAPE.into(),
                design: "botanical".into(),
                steps_title: "南海なんば駅からの道順".into(),
                car_steps: vec![step("c1", "6F ロビー直結の車寄せへ")],
                car_steps_title: "お車の方".into(),
                car_note: "満車の場合は\nなんばシティの駐車場へ".into(),
                floor_map: "floor".into(),
                floor_map_title: "ホテル館内図".into(),
                floor_note: "6F は改装工事中です".into(),
            },
            card: crate::model::CardStyle {
                layout: "photo".into(),
                photo: "couple".into(),
            },
        };
        let defaults = Order::default();
        assert_ne!(order.design, defaults.design);
        assert_ne!(order.options, defaults.options);
        assert_ne!(order.card.layout, defaults.card.layout);
        let back = read_workbook(&write_workbook(&order)).unwrap();
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        // 写真のない説明だけの道順(2 枚目)は説明が空なので読み込みでは落ちる。それ以外はすべて一致する
        let mut expected = settings_only(&order);
        expected.access.steps.retain(|s| !s.caption.is_empty());
        expected.access = expected.access.cleaned();
        expected.event = expected.event.cleaned();
        assert_eq!(settings_only(&back.order), expected);
    }

    #[test]
    fn round_trip_keeps_everything() {
        let order = sample_order();
        let back = read_workbook(&write_workbook(&order)).unwrap();
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        assert!(back.has_settings);
        let got = &back.order;
        assert_eq!(
            (&got.design, &got.event, &got.options),
            (&order.design, &order.event, &order.options)
        );
        assert!(got.event.message.contains('\n'));
        assert_eq!(got.access.routes, order.access.routes, "交通手段も往復で残る");
        assert!(got.access.is_landscape(), "案内図の向きも残る");
        assert_eq!(got.access.design, "sakura", "案内図のデザインも残る");
        assert_eq!(got.access.steps_title, "南海なんば駅からの道順", "道順の見出しも残る");
        assert_eq!(got.access.car_steps_title, "お車の方", "お車の道順の見出しも残る");
        // 道順の説明は「道順」シートで残る(写真は Excel に入らない)
        let captions = |s: &[RouteStep]| s.iter().map(|s| s.caption.clone()).collect::<Vec<_>>();
        assert_eq!(
            captions(&got.access.steps),
            ["北改札を出て赤丸のホテルロゴへ", "矢印先のエレベーターで 6F ロビーへ"]
        );
        assert_eq!(captions(&got.access.car_steps), ["6F ロビー直結の車寄せへ"]);
        assert!(got.access.steps.iter().all(|s| s.photo.is_empty()));
        assert_eq!(got.card.layout, "vertical", "招待状のレイアウトも残る");
        assert_eq!(
            (got.access.title.as_str(), got.access.note.as_str()),
            ("ご案内図", "送迎バスあり\n10:00 発")
        );
        // 宛名に使う内容は同じ(郵便番号は One-W に合わせて数字だけで保存される)
        let printed = |g: &Guest| {
            (
                g.name.clone(),
                g.display_postal(),
                g.address1.clone(),
                g.address2.clone(),
                g.joint_name.clone(),
            )
        };
        assert_eq!(
            got.guests.iter().map(printed).collect::<Vec<_>>(),
            order.guests.iter().map(printed).collect::<Vec<_>>()
        );
        // 2 回目以降の往復では完全に一致する
        let again = read_workbook(&write_workbook(got)).unwrap();
        assert_eq!(&again.order, got);
    }

    #[test]
    fn guest_only_workbook() {
        let order = sample_order();
        let back = read_workbook(&write_guest_workbook(&order.guests)).unwrap();
        assert!(!back.has_settings);
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        assert_eq!(back.order.guests.len(), 2);
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
    fn column_letters() {
        assert_eq!(col_letter(0), "A");
        assert_eq!(col_letter(25), "Z");
        assert_eq!(col_letter(26), "AA");
        assert_eq!(col_letter(31), "AF");
        assert_eq!(cell_ref("AF12"), Some((11, 31)));
    }

    #[test]
    fn not_xlsx() {
        assert!(read_workbook(b"hello").is_err());
        let zip = zip::write(&[("a.txt", b"x")], true);
        assert!(read_workbook(&zip).is_err());
    }
}
