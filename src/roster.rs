//! ゲスト名簿の表(CSV / xlsx 共通).
//!
//! One-W の「出席者名簿」形式(姓・名・肩書き・…・住所1・住所2 …)と、
//! このアプリの簡易形式(氏名・郵便番号・住所1・住所2・連名・敬称)を見出しで見分けて読み込む。
//! One-W 形式では、印刷に使わない列(肩書き・ふりがな・出欠など)も [`Guest::extra`] に残し、
//! 書き出しで元の列に戻す。

use crate::model::Guest;

pub const ONEW_SHEET: &str = "出席者名簿";

/// One-W の出席者名簿の列(この順で書き出す).
pub const ONEW_COLUMNS: [&str; 32] = [
    "姓",
    "名",
    "肩書き",
    "外字ファイル名",
    "姓ふりがな",
    "名ふりがな",
    "敬称",
    "区分",
    "〒",
    "住所1",
    "住所2",
    "携帯電話番号",
    "備考",
    "配席メモ",
    "出欠状態",
    "性別",
    "PC Email",
    "携帯 Email",
    "電話番号",
    "グループ",
    "ふりがな表記",
    "お子様の年齢",
    "未成年",
    "バス",
    "タクシー",
    "宿泊",
    "その他",
    "引出物",
    "引菓子",
    "縁起物",
    "料理",
    "特記事項",
];

/// One-W にない列。連名があるときだけ末尾に付ける.
const JOINT_COLUMN: &str = "連名";

/// 簡易形式の列(見出しのない CSV はこの順とみなす).
const SIMPLE_COLUMNS: [Col; 6] = [
    Col::Name,
    Col::Postal,
    Col::Address1,
    Col::Address2,
    Col::Joint,
    Col::Honorific,
];

#[derive(Debug, Clone, PartialEq)]
pub enum Col {
    Family,
    Given,
    Name,
    Postal,
    Address1,
    Address2,
    Joint,
    Honorific,
    /// そのまま残す列(見出し名)
    Extra(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub onew: bool,
    pub cols: Vec<Option<Col>>,
}

fn simple_col(h: &str) -> Option<Col> {
    Some(match h.to_lowercase().as_str() {
        "氏名" | "名前" | "お名前" | "name" => Col::Name,
        "郵便番号" | "〒" | "postal_code" => Col::Postal,
        "住所1" | "住所" | "address1" => Col::Address1,
        "住所2" | "建物名" | "address2" => Col::Address2,
        "連名" | "joint_name" => Col::Joint,
        "敬称" | "honorific" => Col::Honorific,
        _ => return None,
    })
}

/// 見出し行かどうかを判定し、各列の意味を返す.
pub fn detect(row: &[String]) -> Option<Header> {
    let cells: Vec<&str> = row.iter().map(|c| c.trim()).collect();
    if cells.contains(&"姓") && cells.contains(&"名") {
        let cols = cells
            .iter()
            .map(|h| match *h {
                "" => None,
                "姓" => Some(Col::Family),
                "名" => Some(Col::Given),
                "敬称" => Some(Col::Honorific),
                "〒" | "郵便番号" => Some(Col::Postal),
                "住所1" => Some(Col::Address1),
                "住所2" => Some(Col::Address2),
                JOINT_COLUMN => Some(Col::Joint),
                other => Some(Col::Extra(other.to_string())),
            })
            .collect();
        return Some(Header { onew: true, cols });
    }
    let cols: Vec<Option<Col>> = cells.iter().map(|h| simple_col(h)).collect();
    cols.contains(&Some(Col::Name)).then_some(Header { onew: false, cols })
}

/// 見出しのない CSV 用の既定の列.
pub fn default_header() -> Header {
    Header {
        onew: false,
        cols: SIMPLE_COLUMNS.iter().cloned().map(Some).collect(),
    }
}

/// 数値として保存され先頭の 0 が消えた郵便番号(6 桁)を戻す.
fn normalize_postal(s: &str) -> String {
    let s = s.trim();
    if s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit()) {
        format!("0{s}")
    } else {
        s.to_string()
    }
}

/// One-W の住所2(番地 + 建物名)を番地と建物名に分ける.
///
/// 改行があればそこで、なければ番地に使う文字(数字・ハイフン・丁目番地号)が途切れたところで分ける。
/// 番地で始まらなければ全体を建物名とみなす(このアプリから書き出した住所2は建物名だけのため)。
pub fn split_banchi(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some((a, b)) = s.split_once('\n') {
        return (a.trim().to_string(), b.trim().replace('\n', " "));
    }
    let is_banchi = |c: char| c.is_ascii_digit() || ('０'..='９').contains(&c) || "-－ー‐−―ｰ丁目番地号".contains(c);
    let end = s
        .char_indices()
        .find(|(_, c)| !is_banchi(*c))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    if end == 0 {
        return (String::new(), s.to_string());
    }
    let (a, b) = s.split_at(end);
    (a.to_string(), b.trim().to_string())
}

/// One-W の住所1・住所2 を、このアプリの住所1(番地まで)・住所2(建物名)にする.
fn merge_address(a1: &str, a2: &str) -> (String, String) {
    let (banchi, building) = split_banchi(a2);
    (format!("{}{}", a1.trim(), banchi), building)
}

/// 書き出し用に One-W の住所1・住所2 に戻す.
fn onew_address(g: &Guest) -> (String, String) {
    let orig1 = g.extra.get("住所1").cloned().unwrap_or_default();
    let orig2 = g.extra.get("住所2").cloned().unwrap_or_default();
    // 読み込んだときから住所が変わっていなければ元の値をそのまま使う
    if !(orig1.is_empty() && orig2.is_empty())
        && merge_address(&orig1, &orig2) == (g.address1.clone(), g.address2.clone())
    {
        return (orig1, orig2);
    }
    let join = |a: &str, b: &str| match (a.is_empty(), b.is_empty()) {
        (false, false) => format!("{a} {b}"),
        _ => format!("{a}{b}"),
    };
    match g.address1.strip_prefix(orig1.as_str()) {
        Some(rest) if !orig1.is_empty() => (orig1.clone(), join(rest, &g.address2)),
        _ => (g.address1.clone(), g.address2.clone()),
    }
}

/// 1 行分のセルからゲストを作る.
pub fn guest_from_row(header: &Header, row: &[String]) -> Guest {
    let mut g = Guest::default();
    let (mut family, mut given, mut a1, mut a2) = (String::new(), String::new(), String::new(), String::new());
    for (col, value) in header.cols.iter().zip(row) {
        let value = value.trim().to_string();
        match col {
            None => {}
            Some(Col::Family) => family = value,
            Some(Col::Given) => given = value,
            Some(Col::Name) => g.name = value,
            Some(Col::Postal) => g.postal_code = normalize_postal(&value),
            Some(Col::Address1) => a1 = value,
            Some(Col::Address2) => a2 = value,
            Some(Col::Joint) => g.joint_name = value,
            Some(Col::Honorific) => g.honorific = value,
            Some(Col::Extra(key)) if !value.is_empty() => {
                g.extra.insert(key.clone(), value);
            }
            Some(Col::Extra(_)) => {}
        }
    }
    if header.onew {
        g.name = [family.as_str(), given.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        (g.address1, g.address2) = merge_address(&a1, &a2);
        for (key, value) in [("住所1", a1), ("住所2", a2)] {
            if !value.is_empty() {
                g.extra.insert(key.into(), value);
            }
        }
    } else {
        g.address1 = a1;
        g.address2 = a2;
    }
    g.cleaned()
}

/// 見出しを探してゲスト一覧にする。見出しがなければ None.
pub fn guests_from_table(rows: &[Vec<String>]) -> Option<Vec<Guest>> {
    let (h, header) = rows
        .iter()
        .enumerate()
        .take(10)
        .find_map(|(i, r)| detect(r).map(|h| (i, h)))?;
    Some(
        rows[h + 1..]
            .iter()
            .map(|r| guest_from_row(&header, r))
            .filter(|g| !g.name.is_empty())
            .collect(),
    )
}

/// One-W の出席者名簿の表(見出し行つき).
pub fn onew_table(guests: &[Guest]) -> Vec<Vec<String>> {
    let mut header: Vec<String> = ONEW_COLUMNS.iter().map(|s| s.to_string()).collect();
    // One-W にない列を読み込んでいれば末尾に残す
    for g in guests {
        for key in g.extra.keys() {
            if !header.contains(key) {
                header.push(key.clone());
            }
        }
    }
    if guests.iter().any(|g| !g.joint_name.is_empty()) && !header.iter().any(|h| h == JOINT_COLUMN) {
        header.push(JOINT_COLUMN.into());
    }
    let mut rows = vec![header.clone()];
    for g in guests {
        let (family, given) = g.family_given();
        let (a1, a2) = onew_address(g);
        let postal: String = g.display_postal().chars().filter(|c| c.is_ascii_digit()).collect();
        let postal = if postal.len() == 7 {
            postal
        } else {
            g.postal_code.clone()
        };
        rows.push(
            header
                .iter()
                .map(|h| match h.as_str() {
                    "姓" => family.clone(),
                    "名" => given.clone(),
                    "敬称" => g.honorific.clone(),
                    "〒" => postal.clone(),
                    "住所1" => a1.clone(),
                    "住所2" => a2.clone(),
                    JOINT_COLUMN => g.joint_name.clone(),
                    other => g.extra.get(other).cloned().unwrap_or_default(),
                })
                .collect(),
        );
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn banchi_and_building() {
        let cases = [
            ("2-15-3 アドニス406号", ("2-15-3", "アドニス406号")),
            (
                "1-4-12パークエクセル三国ヶ丘407",
                ("1-4-12", "パークエクセル三国ヶ丘407"),
            ),
            ("6-6-1\n高安グリーンハイツ207号", ("6-6-1", "高安グリーンハイツ207号")),
            ("６０５ー５", ("６０５ー５", "")),
            ("3丁目1番1号", ("3丁目1番1号", "")),
            ("1-8-18 701", ("1-8-18", "701")),
            ("グランドメゾン原宿 302号室", ("", "グランドメゾン原宿 302号室")),
        ];
        for (input, (a, b)) in cases {
            assert_eq!(split_banchi(input), (a.to_string(), b.to_string()), "{input}");
        }
    }

    #[test]
    fn onew_round_trip() {
        let header = row(&ONEW_COLUMNS);
        let mut data = vec![String::new(); ONEW_COLUMNS.len()];
        let set = |data: &mut Vec<String>, col: &str, v: &str| {
            let i = ONEW_COLUMNS.iter().position(|c| *c == col).unwrap();
            data[i] = v.into();
        };
        set(&mut data, "姓", "架空");
        set(&mut data, "名", "太郎");
        set(&mut data, "肩書き", "新郎友人");
        set(&mut data, "姓ふりがな", "かくう");
        set(&mut data, "区分", "新郎側");
        set(&mut data, "〒", "600001");
        set(&mut data, "住所1", "北海道札幌市中央区北一条西");
        set(&mut data, "住所2", "2-3\nサンプルハイツ101号");
        set(&mut data, "出欠状態", "確認中");
        let guests = guests_from_table(&[header.clone(), data.clone()]).unwrap();
        let g = &guests[0];
        assert_eq!(g.name, "架空 太郎");
        assert_eq!(g.postal_code, "0600001");
        assert_eq!(g.address1, "北海道札幌市中央区北一条西2-3");
        assert_eq!(g.address2, "サンプルハイツ101号");
        assert_eq!(g.honorific, "", "空欄の敬称は空欄のまま残す");
        assert_eq!(g.honorific_or_default(), "様");
        assert_eq!(g.extra["肩書き"], "新郎友人");

        let table = onew_table(&guests);
        assert_eq!(table[0], header);
        let mut expected = data;
        set(&mut expected, "〒", "0600001");
        assert_eq!(table[1], expected, "変更していなければ元の住所2(改行入り)のまま");
    }

    #[test]
    fn edited_address_is_split_back() {
        let mut g = Guest::new(
            "架空 花子",
            "100-0001",
            "東京都千代田区千代田1-1",
            "架空ビル 5F",
            "一郎",
        );
        g.extra.insert("住所1".into(), "東京都千代田区千代田".into());
        g.extra.insert("住所2".into(), "9-9".into());
        let table = onew_table(&[g]);
        let col = |name: &str| table[0].iter().position(|h| h == name).unwrap();
        assert_eq!(table[1][col("住所1")], "東京都千代田区千代田");
        assert_eq!(table[1][col("住所2")], "1-1 架空ビル 5F");
        assert_eq!(table[1][col("〒")], "1000001");
        assert_eq!(table[1][col("連名")], "一郎");
    }

    #[test]
    fn simple_format_still_works() {
        let rows = vec![
            row(&["氏名", "郵便番号", "住所1"]),
            row(&["架空 次郎", "1000001", "東京都"]),
        ];
        let g = guests_from_table(&rows).unwrap();
        assert_eq!(g[0].name, "架空 次郎");
        assert!(g[0].extra.is_empty());
        assert!(detect(&row(&["foo", "bar"])).is_none());
    }
}
