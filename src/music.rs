//! 楽曲管理(シーンごとの楽曲と発注状況)の JSON の読み書きと集計.
//!
//! JSON は手で編集しても読めるよう、別名(日本語の項目名・英語の状況名)や
//! 文字列 / 配列のどちらで書かれた楽曲名も受け付けて整える。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 発注状況(この順に進む).
pub const STATUSES: [&str; 4] = ["選定中", "決定", "発注済み", "納品済み"];
pub const SELECTING: &str = "選定中";

pub const MAX_SONGS: usize = 100;
pub const MAX_CANDIDATES: usize = 10;
const MAX_LEN: usize = 200;

const DEFAULT_SCENES: [&str; 12] = [
    "迎賓",
    "新郎新婦入場",
    "乾杯",
    "ケーキ入刀",
    "歓談",
    "お色直し退場",
    "再入場",
    "花嫁の手紙",
    "花束贈呈",
    "謝辞",
    "退場",
    "送賓",
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Song {
    /// 使うシーン
    pub scene: String,
    /// 楽曲名。選定中は候補を複数持てる。それ以外は 1 曲
    pub titles: Vec<String>,
    /// 発注状況(STATUSES のいずれか)
    pub status: String,
    /// 発注先
    pub vendor: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicList {
    pub version: u32,
    pub songs: Vec<Song>,
}

impl Default for MusicList {
    fn default() -> Self {
        MusicList {
            version: 1,
            songs: DEFAULT_SCENES
                .iter()
                .map(|s| Song {
                    scene: s.to_string(),
                    status: SELECTING.into(),
                    ..Song::default()
                })
                .collect(),
        }
    }
}

fn clean(s: &str) -> String {
    s.trim().chars().take(MAX_LEN).collect()
}

/// 状況の表記ゆれ・英語名を正規化する.
pub fn normalize_status(s: &str) -> Option<&'static str> {
    Some(match s.trim().to_lowercase().as_str() {
        "" | "選定中" | "選曲中" | "未定" | "検討中" | "selecting" | "candidate" | "todo" => "選定中",
        "決定" | "決定済み" | "決定済" | "decided" | "selected" => "決定",
        "発注済み" | "発注済" | "発注" | "依頼済み" | "申請済み" | "ordered" => "発注済み",
        "納品済み" | "納品済" | "納品" | "完了" | "delivered" | "done" => "納品済み",
        _ => return None,
    })
}

fn field<'a>(obj: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| obj.get(*k))
}

fn as_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => clean(s),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn as_titles(v: Option<&Value>) -> Vec<String> {
    let list: Vec<String> = match v {
        Some(Value::Array(items)) => items.iter().map(|i| as_text(Some(i))).collect(),
        // 「曲A / 曲B」「曲A、曲B」のような 1 行の書き方も候補に分ける
        Some(Value::String(s)) => s.split(['/', '、', '\n', '|']).map(clean).collect(),
        other => vec![as_text(other)],
    };
    let mut out: Vec<String> = Vec::new();
    for t in list {
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// 結果: 整えた一覧と、直した点の注意.
#[derive(Debug, Serialize)]
pub struct Normalized {
    pub list: MusicList,
    pub warnings: Vec<String>,
}

/// JSON(`{"songs": [...]}` または配列そのもの)を読み、整える.
pub fn normalize(text: &str) -> Result<Normalized, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("JSON として読めません: {e}"))?;
    let items = match &value {
        Value::Array(items) => items.clone(),
        Value::Object(obj) => match field(obj, &["songs", "楽曲", "曲"]) {
            Some(Value::Array(items)) => items.clone(),
            _ => return Err("「songs」の一覧が見つかりません".into()),
        },
        _ => return Err("楽曲の一覧(配列)が見つかりません".into()),
    };
    let mut warnings = Vec::new();
    let mut songs = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let Value::Object(obj) = item else {
            warnings.push(format!("{} 件目は形式が正しくないため読み飛ばしました", i + 1));
            continue;
        };
        let scene = as_text(field(obj, &["scene", "使うシーン", "シーン"]));
        let mut titles = as_titles(field(obj, &["titles", "title", "楽曲名", "曲名", "候補"]));
        let vendor = as_text(field(obj, &["vendor", "発注先"]));
        let raw_status = as_text(field(obj, &["status", "発注状況", "状況"]));
        let status = normalize_status(&raw_status).unwrap_or_else(|| {
            warnings.push(format!(
                "「{scene}」の発注状況「{raw_status}」が分からないため「選定中」にしました"
            ));
            SELECTING
        });
        if scene.is_empty() && titles.is_empty() {
            continue;
        }
        if status != SELECTING && titles.len() > 1 {
            warnings.push(format!(
                "「{scene}」は{status}ですが楽曲が {} 曲あるため、1 曲目の「{}」を残しました",
                titles.len(),
                titles[0]
            ));
            titles.truncate(1);
        }
        if titles.len() > MAX_CANDIDATES {
            warnings.push(format!("「{scene}」の候補は {MAX_CANDIDATES} 曲までです"));
            titles.truncate(MAX_CANDIDATES);
        }
        songs.push(Song {
            scene,
            titles,
            status: status.into(),
            vendor,
        });
    }
    if songs.len() > MAX_SONGS {
        warnings.push(format!("楽曲は {MAX_SONGS} 件までです"));
        songs.truncate(MAX_SONGS);
    }
    Ok(Normalized {
        list: MusicList { version: 1, songs },
        warnings,
    })
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Count {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub total: usize,
    /// STATUSES の順の件数
    pub by_status: Vec<Count>,
    /// 発注先ごとの件数(多い順、未定は最後)
    pub by_vendor: Vec<Count>,
    /// 納品済みの割合(0〜100)
    pub done_percent: u32,
    /// 楽曲がまだ決まっていない(選定中の)シーン
    pub undecided: Vec<String>,
    /// 決定しているが発注先が空のシーン
    pub missing_vendor: Vec<String>,
    /// 決定しているが楽曲名が空のシーン
    pub missing_title: Vec<String>,
}

pub fn summary(list: &MusicList) -> Summary {
    let total = list.songs.len();
    let by_status = STATUSES
        .iter()
        .map(|s| Count {
            label: s.to_string(),
            count: list.songs.iter().filter(|x| x.status == *s).count(),
        })
        .collect();
    let mut by_vendor: Vec<Count> = Vec::new();
    for song in &list.songs {
        let label = if song.vendor.is_empty() {
            "未定".to_string()
        } else {
            song.vendor.clone()
        };
        match by_vendor.iter_mut().find(|c| c.label == label) {
            Some(c) => c.count += 1,
            None => by_vendor.push(Count { label, count: 1 }),
        }
    }
    by_vendor.sort_by_key(|c| (c.label == "未定", std::cmp::Reverse(c.count)));
    let done = list.songs.iter().filter(|s| s.status == "納品済み").count();
    let name = |s: &Song| {
        if s.scene.is_empty() {
            "(シーン未入力)".to_string()
        } else {
            s.scene.clone()
        }
    };
    Summary {
        total,
        by_status,
        by_vendor,
        done_percent: (done * 100).checked_div(total).unwrap_or(0) as u32,
        undecided: list.songs.iter().filter(|s| s.status == SELECTING).map(name).collect(),
        missing_vendor: list
            .songs
            .iter()
            .filter(|s| s.status != SELECTING && s.vendor.is_empty())
            .map(name)
            .collect(),
        missing_title: list
            .songs
            .iter()
            .filter(|s| s.status != SELECTING && s.titles.is_empty())
            .map(name)
            .collect(),
    }
}

pub fn to_json(list: &MusicList) -> String {
    serde_json::to_string_pretty(list).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_round_trip() {
        let list = MusicList::default();
        assert_eq!(list.songs.len(), 12);
        let back = normalize(&to_json(&list)).unwrap();
        assert!(back.warnings.is_empty());
        assert_eq!(back.list, list);
    }

    #[test]
    fn lenient_input() {
        let json = r#"[
            {"使うシーン": "入場", "楽曲名": "曲A / 曲B、曲C", "発注状況": "selecting", "発注先": ""},
            {"scene": "乾杯", "titles": ["曲D", "曲E"], "status": "発注済", "vendor": "会場"},
            {"scene": "退場", "title": "曲F", "status": "謎"},
            "壊れた行",
            {"scene": "", "titles": []}
        ]"#;
        let n = normalize(json).unwrap();
        let s = &n.list.songs;
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].titles, ["曲A", "曲B", "曲C"]);
        assert_eq!(s[0].status, "選定中");
        assert_eq!(
            (s[1].status.as_str(), s[1].titles.len()),
            ("発注済み", 1),
            "選定中以外は 1 曲"
        );
        assert_eq!(s[2].status, "選定中");
        assert_eq!(n.warnings.len(), 3, "{:?}", n.warnings);
        assert!(normalize("{").is_err());
        assert!(normalize(r#"{"x": 1}"#).is_err());
    }

    #[test]
    fn summary_counts() {
        let list = MusicList {
            version: 1,
            songs: vec![
                Song {
                    scene: "入場".into(),
                    titles: vec!["A".into(), "B".into()],
                    status: "選定中".into(),
                    vendor: String::new(),
                },
                Song {
                    scene: "乾杯".into(),
                    titles: vec!["C".into()],
                    status: "決定".into(),
                    vendor: String::new(),
                },
                Song {
                    scene: "退場".into(),
                    titles: vec!["D".into()],
                    status: "納品済み".into(),
                    vendor: "会場".into(),
                },
                Song {
                    scene: "送賓".into(),
                    titles: vec!["E".into()],
                    status: "発注済み".into(),
                    vendor: "会場".into(),
                },
            ],
        };
        let s = summary(&list);
        assert_eq!(s.total, 4);
        assert_eq!(s.by_status.iter().map(|c| c.count).collect::<Vec<_>>(), [1, 1, 1, 1]);
        assert_eq!(
            s.by_vendor[0],
            Count {
                label: "会場".into(),
                count: 2
            }
        );
        assert_eq!(s.by_vendor[1].label, "未定");
        assert_eq!(s.done_percent, 25);
        assert_eq!(s.undecided, ["入場"]);
        assert_eq!(s.missing_vendor, ["乾杯"]);
        assert!(s.missing_title.is_empty());
    }
}
