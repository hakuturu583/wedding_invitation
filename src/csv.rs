//! ゲスト名簿 CSV の読み書き(One-W の出席者名簿形式と簡易形式).

use crate::model::{Guest, MAX_GUESTS};
use crate::roster;

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

/// 名簿 CSV を読み込む。One-W 形式・簡易形式は見出しで見分け、見出しがなければ簡易形式の列順とみなす.
pub fn parse_guests(data: &[u8]) -> Result<Vec<Guest>, String> {
    let rows = parse_rows(&decode(data)?);
    let mut guests = roster::guests_from_table(&rows).unwrap_or_else(|| {
        let header = roster::default_header();
        rows.iter()
            .map(|r| roster::guest_from_row(&header, r))
            .filter(|g| !g.name.is_empty())
            .collect()
    });
    guests.truncate(MAX_GUESTS);
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

fn to_csv(rows: &[Vec<String>]) -> Vec<u8> {
    // Excel で文字化けしないよう BOM 付き UTF-8
    let mut out = String::from("\u{FEFF}");
    for row in rows {
        out += &row.iter().map(|c| quote(c)).collect::<Vec<_>>().join(",");
        out += "\r\n";
    }
    out.into_bytes()
}

/// ゲスト一覧を One-W の出席者名簿形式の CSV にする.
pub fn guests_csv(guests: &[Guest]) -> Vec<u8> {
    to_csv(&roster::onew_table(guests))
}

/// ひな形 CSV(One-W 形式、サンプル入り).
pub fn sample_csv() -> Vec<u8> {
    guests_csv(&sample_guests())
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
        let printed = |g: &Guest| {
            (
                g.name.clone(),
                g.display_postal(),
                g.address1.clone(),
                g.address2.clone(),
            )
        };
        assert_eq!(
            guests.iter().map(printed).collect::<Vec<_>>(),
            sample_guests().iter().map(printed).collect::<Vec<_>>(),
            "One-W 形式で書き出して読み直しても宛名は同じ"
        );
        assert!(String::from_utf8(sample_csv())
            .unwrap()
            .starts_with("\u{FEFF}姓,名,肩書き,"));
    }

    #[test]
    fn shift_jis_and_headerless() {
        let (sjis, _, _) =
            encoding_rs::SHIFT_JIS.encode("氏名,郵便番号,住所1\n佐藤 花子,100-0001,東京都千代田区千代田1-1\n");
        let g = parse_guests(&sjis).unwrap();
        assert_eq!(g[0].address1, "東京都千代田区千代田1-1");
        assert_eq!(g[0].honorific_or_default(), "様");
        let g = parse_guests("高橋 健,1000001,東京都\n".as_bytes()).unwrap();
        assert_eq!(g[0].postal_code, "1000001");
    }

    #[test]
    fn quoted_cells() {
        let rows = parse_rows("a,\"b,c\",\"d\"\"e\"\r\n\"multi\nline\",x\n\n");
        assert_eq!(rows, vec![vec!["a", "b,c", "d\"e"], vec!["multi\nline", "x"]]);
    }
}
