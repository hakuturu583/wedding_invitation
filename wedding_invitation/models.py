"""入力データのモデル."""

from __future__ import annotations

import datetime as dt
from dataclasses import dataclass, field, fields

DEFAULT_MESSAGE = """謹啓　{時候}
皆様にはますますご清祥のこととお慶び申し上げます
このたび私たちは結婚式を挙げることとなりました
つきましては日頃お世話になっている皆様に
感謝の気持ちを込めて
ささやかではございますが小宴を催したく存じます
ご多用中誠に恐縮ではございますが
ぜひご出席いただきたくご案内申し上げます
謹白"""

# 発送する月(挙式のおよそ2か月前)の時候の挨拶
SEASONAL_GREETINGS = {
    1: "新春の候", 2: "余寒の候", 3: "早春の候", 4: "春暖の候",
    5: "新緑の候", 6: "初夏の候", 7: "盛夏の候", 8: "晩夏の候",
    9: "初秋の候", 10: "秋冷の候", 11: "晩秋の候", 12: "初冬の候",
}

WEEKDAYS = "月火水木金土日"

MAX_LEN = 200
MAX_MESSAGE_LEN = 1000


def _clean(value, limit: int = MAX_LEN) -> str:
    if value is None:
        return ""
    return str(value).replace("\r\n", "\n").replace("\r", "\n").strip()[:limit]


def parse_date(value: str) -> dt.date | None:
    try:
        return dt.date.fromisoformat(value)
    except (TypeError, ValueError):
        return None


def format_date(d: dt.date) -> str:
    return f"{d.year}年{d.month}月{d.day}日（{WEEKDAYS[d.weekday()]}）"


def format_time(value: str) -> str:
    """'13:30' -> '午後1時30分'."""
    try:
        t = dt.time.fromisoformat(value)
    except (TypeError, ValueError):
        return value
    ampm = "午前" if t.hour < 12 else "午後"
    hour = t.hour % 12
    return f"{ampm}{hour}時" + (f"{t.minute}分" if t.minute else "")


def send_month(wedding: dt.date) -> tuple[int, int]:
    """招待状を発送するおおよその年月(挙式の2か月前)."""
    month = wedding.month - 2
    year = wedding.year
    if month <= 0:
        month += 12
        year -= 1
    return year, month


@dataclass
class Guest:
    name: str
    postal_code: str = ""
    address1: str = ""
    address2: str = ""
    joint_name: str = ""  # 連名(名のみ、またはフルネーム)
    honorific: str = "様"

    @classmethod
    def from_dict(cls, data: dict) -> "Guest":
        kwargs = {f.name: _clean(data.get(f.name)) for f in fields(cls)}
        kwargs["honorific"] = kwargs["honorific"] or "様"
        return cls(**kwargs)

    @property
    def family_given(self) -> tuple[str, str]:
        parts = self.name.replace("　", " ").split()
        if len(parts) >= 2:
            return parts[0], "".join(parts[1:])
        return self.name, ""

    @property
    def display_postal(self) -> str:
        digits = "".join(ch for ch in self.postal_code.translate(_ZEN2HAN) if ch.isdigit())
        if len(digits) == 7:
            return f"{digits[:3]}-{digits[3:]}"
        return self.postal_code

    def card_line(self) -> str:
        family, given = self.family_given
        main = f"{family} {given}".strip()
        line = f"{main} {self.honorific}"
        if self.joint_name:
            line += f"　{self.joint_name} {self.honorific}"
        return line


_ZEN2HAN = str.maketrans("０１２３４５６７８９－", "0123456789-")


@dataclass
class Event:
    groom_name: str = "田中 一郎"
    bride_name: str = "佐藤 花子"
    date: str = "2027-04-10"
    reception_open_time: str = "10:30"
    ceremony_time: str = "11:00"
    party_time: str = "12:00"
    venue_name: str = "ホテル グランドブライト 3階 宴会場「瑞鳳」"
    venue_address: str = "東京都千代田区丸の内1-2-3"
    venue_tel: str = "03-1234-5678"
    rsvp_deadline: str = "2027-03-10"
    message: str = DEFAULT_MESSAGE
    date_line: str = ""  # 空なら「2027年2月吉日」を自動生成
    sender_postal: str = "100-0005"
    sender_address: str = "東京都千代田区丸の内9-8-7"

    @classmethod
    def from_dict(cls, data: dict | None) -> "Event":
        data = data or {}
        kwargs = {}
        for f in fields(cls):
            if f.name in data:
                limit = MAX_MESSAGE_LEN if f.name == "message" else MAX_LEN
                kwargs[f.name] = _clean(data[f.name], limit)
        return cls(**kwargs)

    @property
    def wedding_date(self) -> dt.date | None:
        return parse_date(self.date)

    def rendered_message(self) -> str:
        d = self.wedding_date
        greeting = SEASONAL_GREETINGS[send_month(d)[1]] if d else ""
        return self.message.replace("{時候}", greeting)

    def rendered_date_line(self) -> str:
        if self.date_line:
            return self.date_line
        d = self.wedding_date
        if not d:
            return "吉日"
        year, month = send_month(d)
        return f"{year}年{month}月吉日"

    def date_text(self) -> str:
        d = self.wedding_date
        return format_date(d) if d else self.date

    def rsvp_text(self) -> str:
        d = parse_date(self.rsvp_deadline)
        return format_date(d) if d else self.rsvp_deadline


@dataclass
class Options:
    crop_marks: bool = True
    envelope_back: bool = True

    @classmethod
    def from_dict(cls, data: dict | None) -> "Options":
        data = data or {}
        return cls(
            crop_marks=bool(data.get("crop_marks", True)),
            envelope_back=bool(data.get("envelope_back", True)),
        )


@dataclass
class Order:
    event: Event = field(default_factory=Event)
    guests: list[Guest] = field(default_factory=list)
    design: str = "classic"
    options: Options = field(default_factory=Options)
