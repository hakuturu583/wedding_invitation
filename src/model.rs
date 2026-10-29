//! 入力データのモデル.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const DEFAULT_MESSAGE: &str = "謹啓　{時候}
皆様にはますますご清祥のこととお慶び申し上げます
このたび私たちは結婚式を挙げることとなりました
つきましては日頃お世話になっている皆様に
感謝の気持ちを込めて
ささやかではございますが小宴を催したく存じます
ご多用中誠に恐縮ではございますが
ぜひご出席いただきたくご案内申し上げます
謹白";

/// 発送する月(挙式のおよそ 2 か月前)の時候の挨拶.
const SEASONAL_GREETINGS: [&str; 12] = [
    "新春の候",
    "余寒の候",
    "早春の候",
    "春暖の候",
    "新緑の候",
    "初夏の候",
    "盛夏の候",
    "晩夏の候",
    "初秋の候",
    "秋冷の候",
    "晩秋の候",
    "初冬の候",
];
const WEEKDAYS: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];

pub const MAX_GUESTS: usize = 500;
const MAX_LEN: usize = 200;
const MAX_MESSAGE_LEN: usize = 1000;
const MAX_EXTRA_COLUMNS: usize = 64;

fn clean(value: &str, limit: usize) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .chars()
        .take(limit)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// "2027-04-10" 形式を解釈する.
    pub fn parse(s: &str) -> Option<Date> {
        let mut it = s.trim().splitn(3, '-');
        let year = it.next()?.parse().ok()?;
        let month: u32 = it.next()?.parse().ok()?;
        let day: u32 = it.next()?.parse().ok()?;
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        if !(1..=12).contains(&month) || day == 0 || day > days[month as usize - 1] {
            return None;
        }
        Some(Date { year, month, day })
    }

    /// 1970-01-01 からの日数から日付を求める(civil_from_days).
    pub fn from_unix_days(days: i64) -> Date {
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + if month <= 2 { 1 } else { 0 }) as i32;
        Date { year, month, day }
    }

    /// "2027-04-10" 形式.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// 0 = 月曜日.
    pub fn weekday(&self) -> usize {
        // days_from_civil (1970-01-01 は木曜日)
        let y = if self.month <= 2 { self.year - 1 } else { self.year } as i64;
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = self.month as i64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146097 + doe - 719468;
        (days + 3).rem_euclid(7) as usize
    }

    pub fn japanese(&self) -> String {
        format!(
            "{}年{}月{}日（{}）",
            self.year,
            self.month,
            self.day,
            WEEKDAYS[self.weekday()]
        )
    }

    /// 招待状を発送するおおよその年月(挙式の 2 か月前).
    pub fn send_month(&self) -> (i32, u32) {
        if self.month > 2 {
            (self.year, self.month - 2)
        } else {
            (self.year - 1, self.month + 10)
        }
    }
}

/// "13:30" -> "午後1時30分".
pub fn format_time(value: &str) -> String {
    let parsed = value.split_once(':').and_then(|(h, m)| {
        let h: u32 = h.trim().parse().ok()?;
        let m: u32 = m.trim().get(..2).unwrap_or(m.trim()).parse().ok()?;
        (h < 24 && m < 60).then_some((h, m))
    });
    let Some((h, m)) = parsed else { return value.to_string() };
    let ampm = if h < 12 { "午前" } else { "午後" };
    let minutes = if m > 0 { format!("{m}分") } else { String::new() };
    format!("{ampm}{}時{minutes}", h % 12)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Guest {
    pub name: String,
    pub postal_code: String,
    pub address1: String,
    pub address2: String,
    /// 連名(名のみ、またはフルネーム)
    pub joint_name: String,
    /// 空欄なら印刷では「様」を使う(名簿には空欄のまま残す)
    pub honorific: String,
    /// 印刷には使わないが名簿の書き出しで残す列(One-W の肩書き・ふりがな・出欠など)
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

impl Guest {
    pub fn new(name: &str, postal: &str, address1: &str, address2: &str, joint: &str) -> Guest {
        Guest {
            name: name.into(),
            postal_code: postal.into(),
            address1: address1.into(),
            address2: address2.into(),
            joint_name: joint.into(),
            honorific: "様".into(),
            extra: BTreeMap::new(),
        }
        .cleaned()
    }

    pub fn cleaned(self) -> Guest {
        Guest {
            name: clean(&self.name, MAX_LEN),
            postal_code: clean(&self.postal_code, 20),
            address1: clean(&self.address1, MAX_LEN),
            address2: clean(&self.address2, MAX_LEN),
            joint_name: clean(&self.joint_name, MAX_LEN),
            honorific: clean(&self.honorific, 10),
            extra: self
                .extra
                .into_iter()
                .take(MAX_EXTRA_COLUMNS)
                .map(|(k, v)| (clean(&k, 50), clean(&v, MAX_LEN)))
                .filter(|(k, v)| !k.is_empty() && !v.is_empty())
                .collect(),
        }
    }

    /// 印刷に使う敬称(空欄なら「様」).
    pub fn honorific_or_default(&self) -> &str {
        if self.honorific.is_empty() {
            "様"
        } else {
            &self.honorific
        }
    }

    /// 姓と名(スペースで区切られていなければ名は空).
    pub fn family_given(&self) -> (String, String) {
        let normalized = self.name.replace('　', " ");
        let mut parts = normalized.split_whitespace();
        let family = parts.next().unwrap_or("").to_string();
        let given: String = parts.collect();
        if given.is_empty() {
            (self.name.clone(), String::new())
        } else {
            (family, given)
        }
    }

    pub fn display_postal(&self) -> String {
        let digits: String = self
            .postal_code
            .chars()
            .filter_map(|c| match c {
                '0'..='9' => Some(c),
                '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32),
                _ => None,
            })
            .collect();
        if digits.len() == 7 {
            format!("{}-{}", &digits[..3], &digits[3..])
        } else {
            self.postal_code.clone()
        }
    }

    pub fn card_line(&self) -> String {
        let (family, given) = self.family_given();
        let main = format!("{family} {given}");
        let honorific = self.honorific_or_default();
        let mut line = format!("{} {honorific}", main.trim());
        if !self.joint_name.is_empty() {
            line += &format!("　{} {honorific}", self.joint_name);
        }
        line
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Event {
    pub groom_name: String,
    pub bride_name: String,
    pub date: String,
    pub reception_open_time: String,
    pub ceremony_time: String,
    pub party_time: String,
    pub venue_name: String,
    pub venue_address: String,
    pub venue_tel: String,
    pub rsvp_deadline: String,
    pub message: String,
    /// 空なら「2027年2月吉日」を自動生成
    pub date_line: String,
    pub sender_postal: String,
    pub sender_address: String,
}

impl Default for Event {
    fn default() -> Self {
        Event {
            groom_name: "田中 一郎".into(),
            bride_name: "佐藤 花子".into(),
            date: "2027-04-10".into(),
            reception_open_time: "10:30".into(),
            ceremony_time: "11:00".into(),
            party_time: "12:00".into(),
            venue_name: "ホテル グランドブライト 3階 宴会場「瑞鳳」".into(),
            venue_address: "東京都千代田区丸の内1-2-3".into(),
            venue_tel: "03-1234-5678".into(),
            rsvp_deadline: "2027-03-10".into(),
            message: DEFAULT_MESSAGE.into(),
            date_line: String::new(),
            sender_postal: "100-0005".into(),
            sender_address: "東京都千代田区丸の内9-8-7".into(),
        }
    }
}

impl Event {
    pub fn cleaned(self) -> Event {
        let c = |s: &str| clean(s, MAX_LEN);
        Event {
            groom_name: c(&self.groom_name),
            bride_name: c(&self.bride_name),
            date: c(&self.date),
            reception_open_time: c(&self.reception_open_time),
            ceremony_time: c(&self.ceremony_time),
            party_time: c(&self.party_time),
            venue_name: c(&self.venue_name),
            venue_address: c(&self.venue_address),
            venue_tel: c(&self.venue_tel),
            rsvp_deadline: c(&self.rsvp_deadline),
            message: clean(&self.message, MAX_MESSAGE_LEN),
            date_line: c(&self.date_line),
            sender_postal: c(&self.sender_postal),
            sender_address: c(&self.sender_address),
        }
    }

    pub fn wedding_date(&self) -> Option<Date> {
        Date::parse(&self.date)
    }

    pub fn rendered_message(&self) -> String {
        let greeting = self
            .wedding_date()
            .map(|d| SEASONAL_GREETINGS[d.send_month().1 as usize - 1])
            .unwrap_or("");
        self.message.replace("{時候}", greeting)
    }

    pub fn rendered_date_line(&self) -> String {
        if !self.date_line.is_empty() {
            return self.date_line.clone();
        }
        match self.wedding_date() {
            Some(d) => {
                let (y, m) = d.send_month();
                format!("{y}年{m}月吉日")
            }
            None => "吉日".into(),
        }
    }

    pub fn date_text(&self) -> String {
        self.wedding_date()
            .map(|d| d.japanese())
            .unwrap_or_else(|| self.date.clone())
    }

    pub fn rsvp_text(&self) -> String {
        Date::parse(&self.rsvp_deadline)
            .map(|d| d.japanese())
            .unwrap_or_else(|| self.rsvp_deadline.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub crop_marks: bool,
    pub envelope_back: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            crop_marks: true,
            envelope_back: true,
        }
    }
}

/// 案内図に載せる交通手段.
pub const ACCESS_MODES: [(&str, &str); 8] = [
    ("train", "電車"),
    ("subway", "地下鉄"),
    ("bus", "バス"),
    ("taxi", "タクシー"),
    ("car", "お車"),
    ("parking", "駐車場"),
    ("walk", "徒歩"),
    ("plane", "飛行機"),
];

pub const PORTRAIT: &str = "portrait";
pub const LANDSCAPE: &str = "landscape";

pub const MAX_ROUTES: usize = 8;
pub const MAX_STEPS: usize = 6;

/// 交通手段ごとの行き方(例: 電車「JR 大阪駅 中央口より徒歩 5 分」).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessRoute {
    /// ACCESS_MODES のキー
    pub mode: String,
    pub text: String,
}

/// 最寄り駅からの道順の 1 枚(写真と説明).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RouteStep {
    /// photos に登録した写真の ID(空なら写真なし)
    pub photo: String,
    pub caption: String,
}

/// 案内図(ハガキ)の内容。会場名・住所・電話番号は Event のものを使う.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Access {
    pub title: String,
    /// 地図の画像(photos の ID)
    pub map_photo: String,
    pub routes: Vec<AccessRoute>,
    pub steps: Vec<RouteStep>,
    /// 補足(例: 送迎バスの時刻)
    pub note: String,
    /// "portrait"(縦 100×148mm)または "landscape"(横 148×100mm)
    pub orientation: String,
}

impl Default for Access {
    fn default() -> Self {
        Access {
            title: "ご案内図".into(),
            map_photo: String::new(),
            routes: vec![
                AccessRoute {
                    mode: "train".into(),
                    text: "JR 東京駅 丸の内中央口より徒歩 5 分".into(),
                },
                AccessRoute {
                    mode: "subway".into(),
                    text: "地下鉄 大手町駅 D1 出口より徒歩 3 分".into(),
                },
                AccessRoute {
                    mode: "taxi".into(),
                    text: "東京駅 丸の内口のタクシー乗り場より約 5 分".into(),
                },
                AccessRoute {
                    mode: "parking".into(),
                    text: "ホテル地下駐車場 100 台(披露宴ご出席の方は 3 時間無料)".into(),
                },
            ],
            steps: Vec::new(),
            note: String::new(),
            orientation: PORTRAIT.into(),
        }
    }
}

impl Access {
    pub fn cleaned(self) -> Access {
        let routes = self
            .routes
            .into_iter()
            .map(|r| AccessRoute {
                mode: if ACCESS_MODES.iter().any(|(k, _)| *k == r.mode) {
                    r.mode
                } else {
                    "train".into()
                },
                text: clean(&r.text, MAX_LEN),
            })
            .filter(|r| !r.text.is_empty())
            .take(MAX_ROUTES)
            .collect();
        let steps = self
            .steps
            .into_iter()
            .map(|s| RouteStep {
                photo: clean(&s.photo, 64),
                caption: clean(&s.caption, MAX_LEN),
            })
            .filter(|s| !(s.photo.is_empty() && s.caption.is_empty()))
            .take(MAX_STEPS)
            .collect();
        Access {
            title: clean(&self.title, 40),
            map_photo: clean(&self.map_photo, 64),
            routes,
            steps,
            note: clean(&self.note, MAX_MESSAGE_LEN),
            orientation: if self.orientation == LANDSCAPE {
                LANDSCAPE
            } else {
                PORTRAIT
            }
            .into(),
        }
    }

    pub fn is_landscape(&self) -> bool {
        self.orientation == LANDSCAPE
    }

    pub fn is_empty(&self) -> bool {
        self.map_photo.is_empty() && self.routes.is_empty() && self.steps.is_empty() && self.note.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Order {
    pub design: String,
    pub event: Event,
    pub guests: Vec<Guest>,
    pub options: Options,
    pub access: Access,
}

impl Default for Order {
    fn default() -> Self {
        Order {
            design: crate::designs::DEFAULT_DESIGN.into(),
            event: Event::default(),
            guests: Vec::new(),
            options: Options::default(),
            access: Access::default(),
        }
    }
}

impl Order {
    /// JSON を解釈し、値を整えて検証する.
    pub fn from_json(json: &str) -> Result<Order, String> {
        let order: Order =
            serde_json::from_str(json).map_err(|e| format!("入力データの形式が正しくありません: {e}"))?;
        order.validated()
    }

    /// 案内図のように宛名を使わない出力向け(ゲストが 0 名でもよい).
    pub fn from_json_without_guests(json: &str) -> Result<Order, String> {
        let order: Order =
            serde_json::from_str(json).map_err(|e| format!("入力データの形式が正しくありません: {e}"))?;
        order.validated_with(false)
    }

    pub fn validated(self) -> Result<Order, String> {
        self.validated_with(true)
    }

    fn validated_with(self, require_guests: bool) -> Result<Order, String> {
        let guests: Vec<Guest> = self
            .guests
            .into_iter()
            .map(Guest::cleaned)
            .filter(|g| !g.name.is_empty())
            .collect();
        if require_guests && guests.is_empty() {
            return Err("ゲストを 1 名以上入力してください".into());
        }
        if guests.len() > MAX_GUESTS {
            return Err(format!("ゲストは {MAX_GUESTS} 名までです"));
        }
        let design = if self.design.is_empty() {
            crate::designs::DEFAULT_DESIGN.into()
        } else {
            self.design
        };
        if crate::designs::find(&design).is_none() {
            return Err("デザインが見つかりません".into());
        }
        Ok(Order {
            design,
            event: self.event.cleaned(),
            guests,
            options: self.options,
            access: self.access.cleaned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        let d = Date::parse("2027-04-10").unwrap();
        assert_eq!(d.japanese(), "2027年4月10日（土）");
        assert_eq!(Date::parse("2026-10-20").unwrap().weekday(), 1); // 火曜日
        assert!(Date::parse("2027-02-30").is_none());
        assert!(Date::parse("abc").is_none());
        assert_eq!(Date::from_unix_days(0).iso(), "1970-01-01");
        assert_eq!(Date::from_unix_days(20739).iso(), "2026-10-13");
        assert_eq!(Date::from_unix_days(-25569).iso(), "1899-12-30");
    }

    #[test]
    fn event_text() {
        let e = Event::default();
        assert_eq!(e.rendered_date_line(), "2027年2月吉日");
        assert!(e.rendered_message().contains("余寒の候"));
        let e = Event {
            date: "2027-01-20".into(),
            ..Event::default()
        };
        assert_eq!(e.rendered_date_line(), "2026年11月吉日");
    }

    #[test]
    fn times() {
        assert_eq!(format_time("09:30"), "午前9時30分");
        assert_eq!(format_time("12:00"), "午後0時");
        assert_eq!(format_time("23:05"), "午後11時5分");
        assert_eq!(format_time("夕方"), "夕方");
    }

    #[test]
    fn guest_helpers() {
        let g = Guest {
            name: "山田　太郎".into(),
            postal_code: "１５０００１２".into(),
            joint_name: "花子".into(),
            ..Guest::default()
        }
        .cleaned();
        assert_eq!(g.family_given(), ("山田".into(), "太郎".into()));
        assert_eq!(g.display_postal(), "150-0012");
        assert_eq!(g.card_line(), "山田 太郎 様　花子 様");
    }

    #[test]
    fn order_validation() {
        assert!(Order::from_json(r#"{"guests": []}"#).is_err());
        assert!(Order::from_json(r#"{"guests": [{"name": "a"}], "design": "nope"}"#).is_err());
        let o = Order::from_json(r#"{"guests": [{"name": " 山田 太郎 "}, {"name": ""}]}"#).unwrap();
        assert_eq!(o.guests.len(), 1);
        assert_eq!(o.guests[0].honorific_or_default(), "様");
        assert_eq!(o.design, "classic");
    }
}
