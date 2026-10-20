//! ゲスト名簿 CSV の読み書き.

use crate::model::{Guest, MAX_GUESTS};

pub const COLUMNS: [&str; 6] = ["氏名", "郵便番号", "住所1", "住所2", "連名", "敬称"];

const SAMPLE_ROWS: [[&str; 6]; 2] = [
    [
        "山田 太郎",
        "150-0001",
        "東京都渋谷区神宮前1-23-45",
        "グランドメゾン原宿 302号室",
        "花子",
        "様",
    ],
    ["鈴木 一子", "530-0001", "大阪府大阪市北区梅田3-1-1", "", "", "様"],
];

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Name,
    Postal,
    Address1,
    Address2,
    Joint,
    Honorific,
}

const DEFAULT_ORDER: [Field; 6] = [
    Field::Name,
    Field::Postal,
    Field::Address1,
    Field::Address2,
    Field::Joint,
    Field::Honorific,
];

/// CSV の見出し(日本語/英語どちらでも可)を項目に対応させる.
fn header_field(cell: &str) -> Option<Field> {
    Some(match cell.trim().to_lowercase().as_str() {
        "name" | "氏名" | "名前" | "お名前" => Field::Name,
        "postal_code" | "郵便番号" | "〒" => Field::Postal,
        "address1" | "住所1" | "住所" => Field::Address1,
        "address2" | "住所2" | "建物名" => Field::Address2,
        "joint_name" | "連名" => Field::Joint,
        "honorific" | "敬称" => Field::Honorific,
        _ => return None,
    })
}

/// UTF-8(BOM 付き含む)と Excel の Shift_JIS(cp932) に対応.
pub fn decode(data: &[u8]) -> Result<String, String> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    if let Ok(s) = std::str::from_utf8(data) {
        return Ok(s.to_string());
    }
    let (text, _, had_errors) = encoding_rs::SHIFT_JIS.decode(data);
    if had_errors {
        return Err("CSV の文字コードを判別できません(UTF-8 または Shift_JIS で保存してください)".into());
    }
    Ok(text.into_owned())
}

/// RFC 4180 形式(ダブルクォート、セル内改行に対応)を行ごとのセルに分解する.
pub fn parse_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    cell.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => cell.push(ch),
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut cell)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            _ => cell.push(ch),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    rows
}

pub fn parse_guests(data: &[u8]) -> Result<Vec<Guest>, String> {
    let rows = parse_rows(&decode(data)?);
    let Some(first) = rows.first() else {
        return Ok(Vec::new());
    };
    let header: Vec<Option<Field>> = first.iter().map(|c| header_field(c)).collect();
    let (header, body): (Vec<Option<Field>>, &[Vec<String>]) = if header.contains(&Some(Field::Name)) {
        (header, &rows[1..])
    } else {
        // 見出しなし: 既定の列順とみなす
        (DEFAULT_ORDER.iter().map(|f| Some(*f)).collect(), &rows[..])
    };
    let guests = body
        .iter()
        .take(MAX_GUESTS)
        .map(|row| {
            let mut g = Guest::default();
            for (field, value) in header.iter().zip(row) {
                let slot = match field {
                    Some(Field::Name) => &mut g.name,
                    Some(Field::Postal) => &mut g.postal_code,
                    Some(Field::Address1) => &mut g.address1,
                    Some(Field::Address2) => &mut g.address2,
                    Some(Field::Joint) => &mut g.joint_name,
                    Some(Field::Honorific) => &mut g.honorific,
                    None => continue,
                };
                *slot = value.clone();
            }
            g.cleaned()
        })
        .filter(|g| !g.name.is_empty())
        .collect();
    Ok(guests)
}

fn quote(cell: &str) -> String {
    if cell.contains([',', '"', '\n']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// ひな形に入れるサンプルのゲスト.
pub fn sample_guests() -> Vec<Guest> {
    SAMPLE_ROWS
        .iter()
        .map(|r| Guest::new(r[0], r[1], r[2], r[3], r[4]))
        .collect()
}

/// ひな形 CSV(Excel で文字化けしないよう BOM 付き UTF-8).
pub fn sample_csv() -> Vec<u8> {
    let mut out = String::from("\u{FEFF}");
    for row in std::iter::once(COLUMNS).chain(SAMPLE_ROWS) {
        out += &row.iter().map(|c| quote(c)).collect::<Vec<_>>().join(",");
        out += "\r\n";
    }
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_round_trip() {
        let guests = parse_guests(&sample_csv()).unwrap();
        let names: Vec<_> = guests.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["山田 太郎", "鈴木 一子"]);
        assert_eq!(guests[0].joint_name, "花子");
    }

    #[test]
    fn shift_jis_and_headerless() {
        let (sjis, _, _) =
            encoding_rs::SHIFT_JIS.encode("氏名,郵便番号,住所1\n佐藤 花子,100-0001,東京都千代田区千代田1-1\n");
        let g = parse_guests(&sjis).unwrap();
        assert_eq!(g[0].address1, "東京都千代田区千代田1-1");
        assert_eq!(g[0].honorific, "様");
        let g = parse_guests("高橋 健,1000001,東京都\n".as_bytes()).unwrap();
        assert_eq!(g[0].postal_code, "1000001");
    }

    #[test]
    fn quoted_cells() {
        let rows = parse_rows("a,\"b,c\",\"d\"\"e\"\r\n\"multi\nline\",x\n\n");
        assert_eq!(rows, vec![vec!["a", "b,c", "d\"e"], vec!["multi\nline", "x"]]);
    }
}
