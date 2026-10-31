//! 出力した PDF が入稿条件を満たしているかを検証する.

use lopdf::{Document, Object};
use wedding_invitation::access::POSTCARD_SIZE;
use wedding_invitation::canvas::MM;
use wedding_invitation::layout::{BLEED, CARD_SIZE, ENVELOPE_SIZE, MARK_AREA};
use wedding_invitation::model::{Access, Event, Guest, Options, Order, RouteStep};
use wedding_invitation::photos;
use wedding_invitation::{build_pdf, build_preview, build_zip, designs, fonts, Doc};

fn order(design: &str, options: Options) -> Order {
    fonts::init_from_repo();
    Order {
        design: design.into(),
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
        options,
        access: Access::default(),
        card: Default::default(),
    }
}

fn rect(doc: &Document, page: &lopdf::Dictionary, key: &[u8]) -> Vec<f32> {
    let obj = page
        .get(key)
        .unwrap_or_else(|_| panic!("{} がありません", String::from_utf8_lossy(key)));
    let arr = match obj {
        Object::Reference(r) => doc.get_object(*r).unwrap().as_array().unwrap().clone(),
        o => o.as_array().unwrap().clone(),
    };
    arr.iter().map(|o| o.as_float().unwrap()).collect()
}

fn approx(a: &[f32], b: &[f32]) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 0.01, "{a:?} != {b:?}");
    }
}

/// 写真が DeviceCMYK の JPEG で、透明(SMask / Mask)を使っていないことを確かめる.
fn assert_images_cmyk(doc: &Document, page_id: lopdf::ObjectId) -> usize {
    let page = doc.get_dictionary(page_id).unwrap();
    let res = match page.get(b"Resources").unwrap() {
        Object::Reference(r) => doc.get_dictionary(*r).unwrap(),
        o => o.as_dict().unwrap(),
    };
    let Ok(xobjects) = res.get(b"XObject") else { return 0 };
    let xobjects = xobjects.as_dict().unwrap();
    for (_, obj) in xobjects.iter() {
        let stream = doc
            .get_object(obj.as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap();
        let dict = &stream.dict;
        assert_eq!(dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Image");
        assert_eq!(dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceCMYK");
        assert_eq!(dict.get(b"Filter").unwrap().as_name().unwrap(), b"DCTDecode");
        assert_eq!(dict.get(b"BitsPerComponent").unwrap().as_i64().unwrap(), 8);
        assert!(
            dict.get(b"SMask").is_err() && dict.get(b"Mask").is_err(),
            "透明を使わない"
        );
    }
    xobjects.len()
}

/// 色の指定が CMYK だけであること、フォントが埋め込まれていることを確かめる.
fn assert_print_ready(data: &[u8]) -> Document {
    let doc = Document::load_mem(data).expect("PDF として読めること");
    for (_, page_id) in doc.get_pages() {
        let content = doc.get_and_decode_page_content(page_id).unwrap();
        let mut has_cmyk = false;
        for op in &content.operations {
            match op.operator.as_str() {
                "k" | "K" => has_cmyk = true,
                "rg" | "RG" | "g" | "G" | "cs" | "CS" | "sc" | "SC" | "scn" | "SCN" | "gs" | "sh" | "BI" => {
                    panic!(
                        "CMYK 以外の色・透明効果・インライン画像の指定があります: {}",
                        op.operator
                    )
                }
                _ => {}
            }
        }
        assert!(has_cmyk);
        assert_images_cmyk(&doc, page_id);
        let fonts = doc.get_page_fonts(page_id).unwrap();
        for (_, font) in fonts {
            assert_eq!(font.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0");
            let descendants = font.get(b"DescendantFonts").unwrap().as_array().unwrap();
            let cid = doc
                .get_object(descendants[0].as_reference().unwrap())
                .unwrap()
                .as_dict()
                .unwrap();
            let fd = doc
                .get_object(cid.get(b"FontDescriptor").unwrap().as_reference().unwrap())
                .unwrap();
            let file = fd
                .as_dict()
                .unwrap()
                .get(b"FontFile2")
                .expect("フォントが埋め込まれていること");
            let stream = doc
                .get_object(file.as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap();
            let data = stream.decompressed_content().unwrap();
            ttf_ok(&data);
        }
    }
    doc
}

fn ttf_ok(data: &[u8]) {
    assert!(data.len() > 100);
    assert_eq!(&data[..4], &[0, 1, 0, 0], "TrueType のサブセットであること");
}

#[test]
fn cards_are_cmyk_with_bleed_and_trim() {
    for d in &designs::DESIGNS {
        let pdf = build_pdf(&order(d.key, Options::default()), Doc::Card).unwrap();
        let doc = assert_print_ready(&pdf);
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2);
        let page = doc.get_dictionary(pages[&1]).unwrap();
        let (w, h) = CARD_SIZE;
        let off = BLEED + MARK_AREA;
        approx(
            &rect(&doc, page, b"MediaBox"),
            &[0.0, 0.0, w + 2.0 * off, h + 2.0 * off],
        );
        approx(&rect(&doc, page, b"TrimBox"), &[off, off, off + w, off + h]);
        approx(
            &rect(&doc, page, b"BleedBox"),
            &[off - BLEED, off - BLEED, off + w + BLEED, off + h + BLEED],
        );
    }
}

#[test]
fn cards_without_crop_marks_are_trim_plus_bleed() {
    let pdf = build_pdf(
        &order(
            "classic",
            Options {
                crop_marks: false,
                envelope_back: true,
            },
        ),
        Doc::Card,
    )
    .unwrap();
    let doc = assert_print_ready(&pdf);
    let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
    let (w, h) = CARD_SIZE;
    approx(
        &rect(&doc, page, b"MediaBox"),
        &[0.0, 0.0, w + 2.0 * BLEED, h + 2.0 * BLEED],
    );
    assert!((BLEED - 3.0 * MM).abs() < 1e-4);
}

#[test]
fn envelopes() {
    for d in &designs::DESIGNS {
        let pdf = build_pdf(&order(d.key, Options::default()), Doc::Envelope).unwrap();
        let doc = assert_print_ready(&pdf);
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 3, "表 2 名分 + 裏面 1 ページ");
        let page = doc.get_dictionary(pages[&1]).unwrap();
        approx(
            &rect(&doc, page, b"MediaBox"),
            &[0.0, 0.0, ENVELOPE_SIZE.0, ENVELOPE_SIZE.1],
        );
    }
    let pdf = build_pdf(
        &order(
            "classic",
            Options {
                crop_marks: true,
                envelope_back: false,
            },
        ),
        Doc::Envelope,
    )
    .unwrap();
    assert_eq!(Document::load_mem(&pdf).unwrap().get_pages().len(), 2);
}

#[test]
fn text_is_extractable() {
    // ToUnicode CMap があれば、印刷所の検版ツールなどで文字を取り出せる
    // (lopdf は CMap の PostScript ヘッダを解釈できないため、中身を直接確かめる)
    let pdf = build_pdf(&order("classic", Options::default()), Doc::Card).unwrap();
    let doc = Document::load_mem(&pdf).unwrap();
    let page_id = doc.get_pages()[&1];
    let fonts = doc.get_page_fonts(page_id).unwrap();
    let mincho = fonts
        .values()
        .find(|f| {
            f.get(b"BaseFont")
                .unwrap()
                .as_name()
                .unwrap()
                .ends_with(b"ShipporiMincho-Regular")
        })
        .unwrap();
    let cmap = doc
        .get_object(mincho.get(b"ToUnicode").unwrap().as_reference().unwrap())
        .unwrap();
    let cmap = String::from_utf8(
        cmap.as_stream()
            .unwrap()
            .decompressed_content()
            .unwrap_or_else(|_| cmap.as_stream().unwrap().content.clone()),
    )
    .unwrap();
    assert!(cmap.contains("<5C71>"), "「山」の対応がある");
}

#[test]
fn long_texts_still_render() {
    let mut o = order("modern", Options::default());
    o.event.message = "あ".repeat(900);
    o.event.venue_name = "とても長い会場名".repeat(10);
    o.guests = vec![Guest::new(
        &"長い名前の人".repeat(3),
        "1234567",
        &"北海道".repeat(30),
        &"ビル".repeat(30),
        &"名前".repeat(5),
    )];
    assert_print_ready(&build_pdf(&o, Doc::Card).unwrap());
    o.design = "sakura".into();
    assert_print_ready(&build_pdf(&o, Doc::Envelope).unwrap());
}

#[test]
fn zip_contains_both_pdfs() {
    let zip = build_zip(&order("botanical", Options::default())).unwrap();
    assert_eq!(&zip[..4], b"PK\x03\x04");
    let text = String::from_utf8_lossy(&zip);
    assert!(text.contains("invitation_cards.pdf") && text.contains("envelopes.pdf"));
    assert_eq!(&zip[zip.len() - 22..zip.len() - 18], b"PK\x05\x06");
}

#[test]
fn preview_svg() {
    let o = order("sakura", Options::default());
    let svg = build_preview(&o, Doc::Envelope, 1).unwrap();
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    assert!(svg.contains("鈴") && svg.contains("ShipporiMincho"));
    let svg = build_preview(&o, Doc::Card, 99).unwrap();
    assert!(svg.contains("鈴木"), "範囲外の番号は最後のゲスト");
}

fn access_order(with_map: bool, steps: usize) -> Order {
    let mut o = order("sakura", Options::default());
    if with_map {
        photos::add(
            "map",
            600,
            400,
            &photos::demo_rgba(600, 400, 0),
            "data:image/jpeg;base64,".into(),
        )
        .unwrap();
        o.access.map_photo = "map".into();
    }
    for i in 0..steps {
        let id = format!("step{i}");
        photos::add(&id, 400, 300, &photos::demo_rgba(400, 300, i as u32 + 1), String::new()).unwrap();
        o.access.steps.push(RouteStep {
            photo: id,
            caption: format!("{} 番目の角を右に曲がります", i + 1),
        });
    }
    o.access.note = "送迎バスがあります".into();
    o
}

#[test]
fn access_map_is_postcard_with_cmyk_photos() {
    let (w, h) = POSTCARD_SIZE;
    let off = BLEED + MARK_AREA;
    // 地図と道順の両方: 表裏 2 ページ
    let pdf = build_pdf(&access_order(true, 4), Doc::Access).unwrap();
    let doc = assert_print_ready(&pdf);
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 2);
    let page = doc.get_dictionary(pages[&1]).unwrap();
    approx(&rect(&doc, page, b"TrimBox"), &[off, off, off + w, off + h]);
    assert_eq!(assert_images_cmyk(&doc, pages[&1]), 1, "表は地図");
    assert_eq!(assert_images_cmyk(&doc, pages[&2]), 4, "裏は道順の写真");
    // どちらか一方なら 1 ページ
    for (map, steps) in [(true, 0), (false, 2), (false, 0)] {
        let pdf = build_pdf(&access_order(map, steps), Doc::Access).unwrap();
        assert_eq!(assert_print_ready(&pdf).get_pages().len(), 1, "{map} {steps}");
    }
    // 地図がなくても、交通手段と道順写真 6 枚を 1 ページに積むと写真が小さくなるので表裏にする
    let pdf = build_pdf(&access_order(false, 6), Doc::Access).unwrap();
    let doc = assert_print_ready(&pdf);
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 2);
    assert_eq!(assert_images_cmyk(&doc, pages[&2]), 6, "裏は道順の写真");
    // お車の道順は別のページ(表・裏のあと)に並べる
    let mut o = access_order(true, 3);
    o.access.car_steps = o.access.steps.clone();
    let pdf = build_pdf(&o, Doc::Access).unwrap();
    let doc = assert_print_ready(&pdf);
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 3);
    assert_eq!(assert_images_cmyk(&doc, pages[&3]), 3, "お車の道順の写真");
    // 館内図は別のページ(道順のあと)に 1 枚
    let mut o = access_order(true, 3);
    o.access.floor_map = "map".into();
    let pdf = build_pdf(&o, Doc::Access).unwrap();
    let doc = assert_print_ready(&pdf);
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 3);
    assert_eq!(assert_images_cmyk(&doc, pages[&3]), 1, "館内図");
    // 交通手段がなければ道順写真 6 枚だけで 1 ページ
    let mut o = access_order(false, 6);
    o.access.routes.clear();
    assert_eq!(
        assert_print_ready(&build_pdf(&o, Doc::Access).unwrap())
            .get_pages()
            .len(),
        1
    );
    // 写真を登録していない ID は描かない(エラーにしない)
    let mut o = access_order(false, 0);
    o.access.steps.push(RouteStep {
        photo: "missing".into(),
        caption: "写真なし".into(),
    });
    assert_print_ready(&build_pdf(&o, Doc::Access).unwrap());
}

#[test]
fn access_preview_and_zip() {
    let o = access_order(true, 2);
    let svg = build_preview(&o, Doc::Access, 0).unwrap();
    assert!(svg.contains("<image") && svg.contains("data:image/jpeg"));
    let zip = build_zip(&o).unwrap();
    assert!(String::from_utf8_lossy(&zip).contains("access_map.pdf"));
    // 案内図が空なら ZIP に入れない
    let mut empty = order("classic", Options::default());
    empty.access = Access {
        routes: Vec::new(),
        ..Access::default()
    };
    assert!(!String::from_utf8_lossy(&build_zip(&empty).unwrap()).contains("access_map.pdf"));
    // 案内図はゲストがいなくても作れる
    let mut no_guests = access_order(false, 1);
    no_guests.guests.clear();
    assert!(build_preview(&no_guests, Doc::Access, 0).is_ok());
    assert!(build_preview(&no_guests, Doc::Card, 0).is_err());
}

#[test]
fn access_map_landscape() {
    let (w, h) = POSTCARD_SIZE;
    let off = BLEED + MARK_AREA;
    for (map, steps, pages) in [(true, 4, 2), (false, 4, 1), (true, 0, 1), (false, 0, 1)] {
        let mut o = access_order(map, steps);
        o.access.orientation = "landscape".into();
        let pdf = build_pdf(&o, Doc::Access).unwrap();
        let doc = assert_print_ready(&pdf);
        let ids = doc.get_pages();
        assert_eq!(ids.len(), pages, "{map} {steps}");
        for id in ids.values() {
            let page = doc.get_dictionary(*id).unwrap();
            approx(&rect(&doc, page, b"TrimBox"), &[off, off, off + h, off + w]);
        }
    }
}

#[test]
fn card_layouts() {
    photos::add("couple", 600, 400, &photos::demo_rgba(600, 400, 2), String::new()).unwrap();
    for d in &designs::DESIGNS {
        for (layout, images) in [("horizontal", 0), ("vertical", 0), ("photo", 1)] {
            let mut o = order(d.key, Options::default());
            o.card = wedding_invitation::model::CardStyle {
                layout: layout.into(),
                photo: "couple".into(),
            };
            let pdf = build_pdf(&o, Doc::Card).unwrap();
            let doc = assert_print_ready(&pdf);
            let pages = doc.get_pages();
            assert_eq!(pages.len(), 2);
            assert_eq!(assert_images_cmyk(&doc, pages[&1]), images, "{} {layout}", d.key);
        }
    }
    // 写真を選んでいない写真入りは横書きと同じ(写真なし)
    let mut o = order("classic", Options::default());
    o.card.layout = "photo".into();
    let doc = assert_print_ready(&build_pdf(&o, Doc::Card).unwrap());
    assert_eq!(assert_images_cmyk(&doc, doc.get_pages()[&1]), 0);
}

#[test]
fn access_map_has_its_own_design() {
    // 招待状(sakura)とは別に、案内図だけ 7 種類のどれでも選べる
    for orientation in ["portrait", "landscape"] {
        for d in &designs::DESIGNS {
            let mut o = access_order(true, 4);
            o.access.orientation = orientation.into();
            o.access.design = d.key.into();
            let pdf = build_pdf(&o, Doc::Access).unwrap();
            assert_print_ready(&pdf);
        }
    }
    // 知らないデザインは招待状と同じに戻す
    let mut o = access_order(false, 0);
    o.access.design = "unknown".into();
    assert_eq!(o.validated().unwrap().access.design, "");
}
