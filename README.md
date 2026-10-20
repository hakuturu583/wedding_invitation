# 結婚式招待状メーカー

ゲストの名前・住所とデザインを入れると、**印刷所に入稿できる CMYK の PDF** を作る Web アプリです。
Rust で書いて WebAssembly にコンパイルしてあり、**処理はすべてブラウザの中で行います**(サーバー不要。入力した名簿は外部に送信されません)。

## できること

- **6 種類のデザイン**: クラシック(デフォルト)/ボタニカル/桜/モダン/睦月(一月: 紅白の梅・松・金の水引)/雪(冬: 銀鼠の地に雪輪文様)
- **招待状カード**: 105×148mm(A6)。ゲストごとに「〇〇 様」を入れて 1 名 1 ページで出力
  - 本文、日時、会場、返信期日を自動でレイアウト(入りきらないときは文字を自動で縮小)
  - 時候の挨拶 `{時候}` と「〇年〇月吉日」は挙式日から自動で入る
- **封筒(洋形2号 114×162mm)**: 縦書きの宛名。番地は漢数字に変換、連名にも対応(敬称の高さを揃える)、裏面に差出人
- **ゲスト名簿の CSV 読み込み**: UTF-8 と Excel の Shift_JIS のどちらでも読める
- プレビュー(SVG)を見ながら編集でき、入力内容はブラウザに保存される

## 入稿データの仕様

| 項目 | 内容 |
| --- | --- |
| 色空間 | すべて DeviceCMYK(`k`/`K` オペレータのみ。RGB・グレー・透明効果・画像は使っていない) |
| フォント | しっぽり明朝 / Great Vibes を TrueType のサブセットとして埋め込み(ToUnicode 付き) |
| 塗り足し | 招待状カードは各辺 3mm |
| トンボ | 日本式トンボ(内トンボ・外トンボ・センタートンボ)。オン/オフ切り替え可 |
| ページボックス | MediaBox / TrimBox / BleedBox を設定済み |

小さい文字は K のみ、またはそれに近い少ない版で刷るようにし、総インキ量は 300% 以下に抑えています。
封筒は既製の封筒に印刷する想定なので、塗り足しなしの仕上がりサイズで出力します。

> ICC プロファイル(OutputIntent)は入れていないため、PDF/X-1a の規格には準拠していません。
> 印刷所によっては PDF/X での入稿を求められることがあるので、入稿先の規定を確認してください。

## ビルドと起動

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129   # Cargo.toml の wasm-bindgen と同じ版
./build.sh                                          # www/pkg に wasm を出力
python3 -m http.server -d www 8000                  # http://localhost:8000 を開く
```

`www/` は静的ファイルだけなので、GitHub Pages などにそのまま置けます。
`.github/workflows/pages.yml` は main への push でテストとビルドを行い、GitHub Pages に公開します
(リポジトリの Settings → Pages で Source を「GitHub Actions」にしてください)。

## テスト

```bash
cargo test
cargo run --example sample -- target/samples   # 全デザインのサンプル PDF / SVG を書き出す
```

テストでは出力した PDF を読み直し、CMYK 以外の色の指定や画像・透明効果が含まれていないこと、
フォントが埋め込まれていること、MediaBox / TrimBox / BleedBox の値を確かめています。

## 構成

| ファイル | 役割 |
| --- | --- |
| `src/canvas.rs` | 描画命令(ディスプレイリスト)。色は `Cmyk` 型だけ |
| `src/pdf.rs` | ディスプレイリスト → 入稿用 PDF(pdf-writer + subsetter) |
| `src/svg.rs` | ディスプレイリスト → 画面プレビュー用 SVG |
| `src/layout.rs` | 招待状カード・封筒のレイアウト、トンボ |
| `src/designs.rs` | デザイン定義 |
| `src/typeset.rs` | 禁則つき折り返し・縦書き |
| `src/model.rs` / `src/csv.rs` / `src/zip.rs` | 入力データ、CSV 読み込み、ZIP 作成 |
| `src/lib.rs` | JavaScript から呼ぶ API(wasm-bindgen) |
| `www/` | 画面(HTML / CSS / JS)とフォント |

## CSV の形式

見出し行: `氏名,郵便番号,住所1,住所2,連名,敬称`(英語の `name,postal_code,address1,address2,joint_name,honorific` も使える)。
画面の「CSV のひな形」からサンプルをダウンロードできます。

## ライセンス

同梱フォントは SIL Open Font License です(`www/fonts/OFL-*.txt`)。
