"""ゲスト名簿 CSV の読み込み."""

from __future__ import annotations

import csv
import io

from .models import Guest

MAX_GUESTS = 500

# CSV の見出し(日本語/英語どちらでも可) -> Guest のフィールド
HEADERS = {
    "name": "name", "氏名": "name", "名前": "name", "お名前": "name",
    "postal_code": "postal_code", "郵便番号": "postal_code", "〒": "postal_code",
    "address1": "address1", "住所1": "address1", "住所": "address1",
    "address2": "address2", "住所2": "address2", "建物名": "address2",
    "joint_name": "joint_name", "連名": "joint_name",
    "honorific": "honorific", "敬称": "honorific",
}
COLUMNS = ["氏名", "郵便番号", "住所1", "住所2", "連名", "敬称"]

SAMPLE_ROWS = [
    ["山田 太郎", "150-0001", "東京都渋谷区神宮前1-23-45", "グランドメゾン原宿 302号室", "花子", "様"],
    ["鈴木 一子", "530-0001", "大阪府大阪市北区梅田3-1-1", "", "", "様"],
]


def decode(data: bytes) -> str:
    """UTF-8(BOM 付き含む)と Excel の Shift_JIS(cp932) に対応."""
    for enc in ("utf-8-sig", "cp932"):
        try:
            return data.decode(enc)
        except UnicodeDecodeError:
            continue
    raise ValueError("CSV の文字コードを判別できません(UTF-8 または Shift_JIS で保存してください)")


def parse_csv(data: bytes) -> list[Guest]:
    rows = list(csv.reader(io.StringIO(decode(data))))
    rows = [r for r in rows if any(cell.strip() for cell in r)]
    if not rows:
        return []
    header = [HEADERS.get(cell.strip().lower(), HEADERS.get(cell.strip())) for cell in rows[0]]
    if "name" in header:
        body = rows[1:]
    else:  # 見出しなし: 既定の列順とみなす
        header = ["name", "postal_code", "address1", "address2", "joint_name", "honorific"]
        body = rows
    guests = []
    for row in body[:MAX_GUESTS]:
        record = {key: value for key, value in zip(header, row) if key}
        if record.get("name", "").strip():
            guests.append(Guest.from_dict(record))
    return guests


def sample_csv() -> bytes:
    buf = io.StringIO()
    writer = csv.writer(buf)
    writer.writerow(COLUMNS)
    writer.writerows(SAMPLE_ROWS)
    return buf.getvalue().encode("utf-8-sig")
