//! サンプルの PDF とプレビュー SVG を書き出す: cargo run --example sample -- <出力先>

use wedding_invitation::model::{Event, Guest, Options, Order};
use wedding_invitation::{build_pdf, build_preview, designs, fonts, Doc};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "target/samples".into());
    std::fs::create_dir_all(&out).unwrap();
    fonts::init_from_repo();
    for d in &designs::DESIGNS {
        let order = Order {
            design: d.key.into(),
            event: Event::default(),
            guests: vec![
                Guest::new(
                    "山田 太郎",
                    "1500001",
                    "東京都渋谷区神宮前1-23-45",
                    "グランドメゾン原宿 302号室",
                    "花子",
                ),
                Guest::new("鈴木 一子", "530-0001", "大阪府大阪市北区梅田3丁目1番1号", "", ""),
            ],
            options: Options::default(),
        };
        for (doc, name) in [(Doc::Card, "card"), (Doc::Envelope, "envelope")] {
            std::fs::write(format!("{out}/{name}_{}.pdf", d.key), build_pdf(&order, doc).unwrap()).unwrap();
            std::fs::write(
                format!("{out}/{name}_{}.svg", d.key),
                build_preview(&order, doc, 0).unwrap(),
            )
            .unwrap();
        }
    }
    std::fs::write(format!("{out}/template.xlsx"), wedding_invitation::xlsx::template()).unwrap();
    println!("{out} に書き出しました");
}
