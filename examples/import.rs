//! xlsx / csv の名簿を読み込んで結果を JSON で表示する: cargo run --example import -- <ファイル> [書き出し先.xlsx|.csv]
//!
//! 書き出し先を付けると、読み込んだ名簿を One-W の出席者名簿形式で書き出す。

fn main() {
    let path = std::env::args().nth(1).expect("ファイルを指定してください");
    let data = std::fs::read(&path).unwrap();
    let guests = if path.ends_with(".csv") {
        let guests = wedding_invitation::csv::parse_guests(&data).unwrap();
        println!("{}", serde_json::to_string_pretty(&guests).unwrap());
        guests
    } else {
        let imported = wedding_invitation::xlsx::read_workbook(&data).unwrap();
        println!("{}", serde_json::to_string_pretty(&imported).unwrap());
        imported.order.guests
    };
    if let Some(out) = std::env::args().nth(2) {
        let bytes = if out.ends_with(".csv") {
            wedding_invitation::csv::guests_csv(&guests)
        } else {
            wedding_invitation::xlsx::write_guest_workbook(&guests)
        };
        std::fs::write(out, bytes).unwrap();
    }
}
