//! 表計算ソフトで編集・保存した xlsx を読み込めることを確かめる.
//!
//! fixtures/edited_in_excel.xlsx は、このアプリが書き出したテンプレートを openpyxl で開き、
//! Excel で編集したときと同じように日付・時刻・数値のセルを入れて保存したもの
//! (共有文字列の読み込みは xlsx.rs の単体テストで確かめている)。

use wedding_invitation::xlsx::read_workbook;

#[test]
fn reads_workbook_edited_in_spreadsheet_app() {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/edited_in_excel.xlsx"
    ))
    .unwrap();
    let imported = read_workbook(&data).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let o = imported.order;
    assert_eq!(o.design, "mutsuki");
    assert_eq!(o.event.date, "2027-01-23");
    assert_eq!(o.event.reception_open_time, "10:30");
    assert_eq!(o.event.ceremony_time, "11:00");
    assert_eq!(o.event.rsvp_deadline, "2026-12-20");
    assert!(o.event.message.starts_with("謹啓"));
    assert!(!o.options.crop_marks);
    assert!(o.options.envelope_back);
    let names: Vec<_> = o.guests.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["山田 太郎", "鈴木 一子", "高橋 健"]);
    assert_eq!(
        o.guests[0].postal_code, "0600001",
        "数値になった郵便番号の先頭の 0 を戻す"
    );
    assert_eq!(o.guests[2].postal_code, "1000001");
    assert_eq!(o.guests[2].honorific_or_default(), "様");
}

/// One-W の「出席者名簿」形式(架空のデータ)。郵便番号や番地が数値で保存され、
/// 住所2 に番地と建物名が入っている.
#[test]
fn reads_and_writes_onew_guest_list() {
    use wedding_invitation::xlsx::write_guest_workbook;

    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/onew_guests.xlsx")).unwrap();
    let imported = read_workbook(&data).unwrap();
    assert!(!imported.has_settings, "名簿だけのファイル");
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let g = &imported.order.guests;
    assert_eq!(g.len(), 5);
    assert_eq!(g[0].name, "架空 一郎");
    assert_eq!(g[0].honorific, "");
    assert_eq!(g[0].extra["肩書き"], "新郎父");
    assert_eq!(g[1].postal_code, "0600001");
    assert_eq!(g[1].address1, "北海道札幌市中央区北一条西614");
    assert_eq!(
        (g[2].address1.as_str(), g[2].address2.as_str()),
        ("大阪府大阪市北区梅田1-2-3", "サンプルハイツ101号")
    );
    assert_eq!(
        (g[3].address1.as_str(), g[3].address2.as_str()),
        ("兵庫県神戸市中央区加納町4-5-6", "架空タワー2406号室")
    );
    assert_eq!(g[4].address1, "");

    // 書き出して読み直しても同じ(One-W の列・値が残る)
    let again = read_workbook(&write_guest_workbook(g)).unwrap();
    assert_eq!(&again.order.guests, g);
}
