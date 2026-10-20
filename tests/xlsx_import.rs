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
    assert_eq!(o.guests[2].honorific, "様");
}
