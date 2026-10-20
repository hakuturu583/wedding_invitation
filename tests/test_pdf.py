import io
import re
import zipfile

import pytest
from pypdf import PdfReader
from reportlab.lib.units import mm

from wedding_invitation.app import create_app
from wedding_invitation.designs import DESIGNS
from wedding_invitation.guests import parse_csv, sample_csv
from wedding_invitation.models import Event, Guest, Options, format_time
from wedding_invitation.pdf import BLEED, CARD_SIZE, ENVELOPE_SIZE, MARK_AREA, render_cards, render_envelopes
from wedding_invitation.typeset import to_vertical, wrap, MINCHO, register_fonts

GUESTS = [
    Guest("山田 太郎", "1500001", "東京都渋谷区神宮前1-23-45", "グランドメゾン原宿 302号室", "花子"),
    Guest("鈴木 一子", "530-0001", "大阪府大阪市北区梅田3丁目1番1号"),
]

# DeviceRGB / DeviceGray / 任意色空間を指定するオペレータ
NON_CMYK_OPS = re.compile(rb"(?<![A-Za-z])(rg|RG|g|G|cs|CS|sc|SC|scn|SCN)\s")


def _assert_print_ready(data: bytes):
    reader = PdfReader(io.BytesIO(data))
    for page in reader.pages:
        content = page.get_contents().get_data()
        # 文字列リテラルを除去してからオペレータを検査
        ops = re.sub(rb"\((?:\\.|[^\\)])*\)|<[0-9A-Fa-f]*>", b"", content)
        assert not NON_CMYK_OPS.search(ops), "CMYK 以外の色指定が含まれています"
        assert b" k" in ops or b" K" in ops
        assert "/XObject" not in page["/Resources"], "画像は含めない"
        for font in page["/Resources"]["/Font"].values():
            font = font.get_object()
            desc = font.get("/FontDescriptor")
            assert desc is not None, f"フォント {font['/BaseFont']} が埋め込まれていません"
            assert "/FontFile2" in desc.get_object()
    return reader


@pytest.mark.parametrize("design", DESIGNS)
def test_cards_are_cmyk_with_bleed_and_trim(design):
    data = render_cards(Event(), GUESTS, design, Options())
    reader = _assert_print_ready(data)
    assert len(reader.pages) == len(GUESTS)
    page = reader.pages[0]
    w, h = CARD_SIZE
    off = BLEED + MARK_AREA
    assert float(page.mediabox.width) == pytest.approx(w + 2 * off)
    trim = [float(v) for v in page.trimbox]
    assert trim == pytest.approx([off, off, off + w, off + h])
    bleed = [float(v) for v in page.bleedbox]
    assert bleed == pytest.approx([off - BLEED, off - BLEED, off + w + BLEED, off + h + BLEED])


def test_cards_without_crop_marks_are_trim_plus_bleed():
    data = render_cards(Event(), GUESTS[:1], "classic", Options(crop_marks=False))
    page = PdfReader(io.BytesIO(data)).pages[0]
    assert float(page.mediabox.width) == pytest.approx(CARD_SIZE[0] + 2 * BLEED)
    assert float(page.mediabox.height) == pytest.approx(CARD_SIZE[1] + 2 * BLEED)


@pytest.mark.parametrize("design", DESIGNS)
def test_envelopes(design):
    data = render_envelopes(Event(), GUESTS, design, Options())
    reader = _assert_print_ready(data)
    assert len(reader.pages) == len(GUESTS) + 1  # 裏面 1 ページ
    page = reader.pages[0]
    assert float(page.mediabox.width) == pytest.approx(ENVELOPE_SIZE[0])
    assert float(page.mediabox.height) == pytest.approx(ENVELOPE_SIZE[1])
    no_back = render_envelopes(Event(), GUESTS, design, Options(envelope_back=False))
    assert len(PdfReader(io.BytesIO(no_back)).pages) == len(GUESTS)


def test_long_texts_still_render():
    event = Event(message="あ" * 900, venue_name="とても長い会場名" * 10)
    guest = Guest("長い名前の人" * 3, "1234567", "北海道" * 30, "ビル" * 30, "名前" * 5)
    assert render_cards(event, [guest], "modern", Options())
    assert render_envelopes(event, [guest], "sakura", Options())


def test_vertical_normalization():
    assert to_vertical("1-23-45 A棟") == "一－二三－四五 Ａ棟"


def test_wrap_kinsoku():
    register_fonts()
    lines = wrap("あいうえお。かきくけこ", MINCHO, 10, 50)
    assert all(not line.startswith("。") for line in lines)


def test_format_time_and_event_text():
    assert format_time("09:30") == "午前9時30分"
    assert format_time("12:00") == "午後0時"
    event = Event(date="2027-04-10")
    assert event.date_text() == "2027年4月10日（土）"
    assert event.rendered_date_line() == "2027年2月吉日"
    assert "余寒の候" in event.rendered_message()
    assert Event(date="2027-01-20").rendered_date_line() == "2026年11月吉日"


def test_guest_card_line_and_postal():
    g = Guest("山田　太郎", "１５０００１２", joint_name="花子")
    assert g.family_given == ("山田", "太郎")
    assert g.display_postal == "150-0012"
    assert g.card_line() == "山田 太郎 様　花子 様"


def test_csv_parsing_utf8_and_sjis():
    guests = parse_csv(sample_csv())
    assert [g.name for g in guests] == ["山田 太郎", "鈴木 一子"]
    sjis = "氏名,郵便番号,住所1\n佐藤 花子,100-0001,東京都千代田区千代田1-1\n".encode("cp932")
    guests = parse_csv(sjis)
    assert guests[0].address1 == "東京都千代田区千代田1-1"
    assert guests[0].honorific == "様"
    headerless = "高橋 健,1000001,東京都\n".encode()
    assert parse_csv(headerless)[0].postal_code == "1000001"


# ------------------------------------------------------------------ API

@pytest.fixture
def client():
    return create_app().test_client()


def _payload(**kw):
    body = {
        "design": "sakura",
        "event": {"groom_name": "田中 一郎", "bride_name": "佐藤 花子", "date": "2027-05-01"},
        "guests": [{"name": "山田 太郎", "postal_code": "1500001", "address1": "東京都渋谷区"}],
        "options": {"crop_marks": True, "envelope_back": True},
    }
    body.update(kw)
    return body


def test_index(client):
    res = client.get("/")
    assert res.status_code == 200
    assert "招待状" in res.get_data(as_text=True)


@pytest.mark.parametrize("doc", ["card", "envelope"])
def test_preview_and_generate(client, doc):
    res = client.post(f"/api/preview/{doc}", json=_payload())
    assert res.status_code == 200 and res.mimetype == "image/png"
    res = client.post(f"/api/generate/{doc}", json=_payload())
    assert res.status_code == 200
    assert "attachment" in res.headers["Content-Disposition"]
    _assert_print_ready(res.data)


def test_generate_zip(client):
    res = client.post("/api/generate/zip", json=_payload())
    assert res.status_code == 200
    with zipfile.ZipFile(io.BytesIO(res.data)) as zf:
        assert sorted(zf.namelist()) == ["envelopes.pdf", "invitation_cards.pdf"]


def test_validation(client):
    assert client.post("/api/generate/card", json=_payload(guests=[])).status_code == 400
    assert client.post("/api/generate/card", json=_payload(design="nope")).status_code == 400
    assert client.post("/api/generate/pdfx", json=_payload()).status_code == 400
    assert client.post("/api/generate/card", data="x").status_code == 400


def test_csv_upload(client):
    res = client.post("/api/guests/csv", data={"file": (io.BytesIO(sample_csv()), "g.csv")})
    assert res.status_code == 200
    assert res.json["guests"][0]["name"] == "山田 太郎"
    assert client.get("/api/guests/sample.csv").status_code == 200
