//! サンプルの PDF とプレビュー SVG を書き出す: cargo run --example sample -- <出力先>

use wedding_invitation::model::{Access, Event, Guest, Options, Order, RouteStep};
use wedding_invitation::{build_pdf, build_preview, designs, fonts, photos, Doc};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "target/samples".into());
    std::fs::create_dir_all(&out).unwrap();
    fonts::init_from_repo();
    // 合成画像の地図と道順の写真
    photos::add("map", 1200, 800, &photos::demo_rgba(1200, 800, 0), String::new()).unwrap();
    for i in 1..=4 {
        photos::add(
            &format!("step{i}"),
            800,
            600,
            &photos::demo_rgba(800, 600, i),
            String::new(),
        )
        .unwrap();
    }
    let captions = [
        "JR 東京駅の丸の内中央口を出ます",
        "駅前広場を左に進み、横断歩道を渡ります",
        "赤い看板のある角を右に曲がります",
        "ホテルの正面玄関に到着です",
    ];
    let access = Access {
        map_photo: "map".into(),
        steps: captions
            .iter()
            .enumerate()
            .map(|(i, c)| RouteStep {
                photo: format!("step{}", i + 1),
                caption: c.to_string(),
            })
            .collect(),
        note: "送迎バス: 東京駅 八重洲口より 10:00 / 10:20 発(所要約 10 分)".into(),
        ..Access::default()
    };
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
            access: access.clone(),
        };
        for (doc, name) in [
            (Doc::Card, "card"),
            (Doc::Envelope, "envelope"),
            (Doc::Access, "access"),
        ] {
            std::fs::write(format!("{out}/{name}_{}.pdf", d.key), build_pdf(&order, doc).unwrap()).unwrap();
            std::fs::write(
                format!("{out}/{name}_{}.svg", d.key),
                build_preview(&order, doc, 0).unwrap(),
            )
            .unwrap();
        }
    }
    // 横向きの案内図(地図と道順写真の両方 / 道順写真だけ)
    for (name, with_map) in [("access_landscape", true), ("access_landscape_steps", false)] {
        let mut access = access.clone();
        access.orientation = "landscape".into();
        if !with_map {
            access.map_photo.clear();
        }
        let order = Order {
            design: "classic".into(),
            access,
            ..Order::default()
        };
        std::fs::write(format!("{out}/{name}.pdf"), build_pdf(&order, Doc::Access).unwrap()).unwrap();
    }
    std::fs::write(format!("{out}/template.xlsx"), wedding_invitation::xlsx::template()).unwrap();
    println!("{out} に書き出しました");
}
