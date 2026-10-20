"""Flask Web アプリ."""

from __future__ import annotations

import io
import zipfile
from dataclasses import asdict
from urllib.parse import quote

import pypdfium2 as pdfium
from flask import Flask, jsonify, render_template, request, send_file

from .designs import DEFAULT_DESIGN, DESIGNS
from .guests import MAX_GUESTS, parse_csv, sample_csv
from .models import Event, Guest, Options, Order
from .pdf import render_cards, render_envelopes

DOCS = {
    "card": ("invitation_cards.pdf", render_cards),
    "envelope": ("envelopes.pdf", render_envelopes),
}


class BadRequest(ValueError):
    pass


def parse_order(payload: dict | None) -> Order:
    if not isinstance(payload, dict):
        raise BadRequest("リクエストの形式が正しくありません")
    raw_guests = payload.get("guests") or []
    if not isinstance(raw_guests, list):
        raise BadRequest("guests はリストで指定してください")
    guests = [Guest.from_dict(g) for g in raw_guests if isinstance(g, dict)]
    guests = [g for g in guests if g.name]
    if not guests:
        raise BadRequest("ゲストを 1 名以上入力してください")
    if len(guests) > MAX_GUESTS:
        raise BadRequest(f"ゲストは {MAX_GUESTS} 名までです")
    design = payload.get("design") or DEFAULT_DESIGN
    if design not in DESIGNS:
        raise BadRequest("デザインが見つかりません")
    return Order(
        event=Event.from_dict(payload.get("event")),
        guests=guests,
        design=design,
        options=Options.from_dict(payload.get("options")),
    )


def _pdf_response(data: bytes, filename: str, download: bool):
    return send_file(io.BytesIO(data), mimetype="application/pdf",
                     as_attachment=download, download_name=filename)


def _pdf_to_png(data: bytes, scale: float = 2.0) -> bytes:
    """プレビュー用に 1 ページ目を PNG 化する(画面表示のみ。入稿データは PDF)."""
    pdf = pdfium.PdfDocument(data)
    try:
        image = pdf[0].render(scale=scale).to_pil()
    finally:
        pdf.close()
    out = io.BytesIO()
    image.save(out, format="PNG", optimize=True)
    return out.getvalue()


def create_app() -> Flask:
    app = Flask(__name__)
    app.config["MAX_CONTENT_LENGTH"] = 2 * 1024 * 1024

    @app.errorhandler(BadRequest)
    def _bad_request(err):
        return jsonify(error=str(err)), 400

    @app.get("/")
    def index():
        designs = [
            {"key": d.key, "name": d.name, "description": d.description, "swatches": d.swatches()}
            for d in DESIGNS.values()
        ]
        return render_template("index.html", designs=designs, default_event=asdict(Event()),
                               default_design=DEFAULT_DESIGN, max_guests=MAX_GUESTS)

    @app.post("/api/preview/<doc>")
    def preview(doc: str):
        """先頭(または指定した)ゲスト 1 名分のプレビュー画像 (PNG) を返す."""
        if doc not in DOCS:
            raise BadRequest("不明な出力種別です")
        order = parse_order(request.get_json(silent=True))
        index = request.args.get("guest", default=0, type=int)
        guest = order.guests[min(max(index, 0), len(order.guests) - 1)]
        options = Options(crop_marks=order.options.crop_marks, envelope_back=False)
        filename, render = DOCS[doc]
        png = _pdf_to_png(render(order.event, [guest], order.design, options))
        return send_file(io.BytesIO(png), mimetype="image/png")

    @app.post("/api/generate/<doc>")
    def generate(doc: str):
        order = parse_order(request.get_json(silent=True))
        if doc in DOCS:
            filename, render = DOCS[doc]
            data = render(order.event, order.guests, order.design, order.options)
            return _pdf_response(data, filename, True)
        if doc == "zip":
            buf = io.BytesIO()
            with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
                for filename, render in DOCS.values():
                    zf.writestr(filename, render(order.event, order.guests, order.design, order.options))
            buf.seek(0)
            return send_file(buf, mimetype="application/zip", as_attachment=True,
                             download_name="wedding_invitation_print_data.zip")
        raise BadRequest("不明な出力種別です")

    @app.post("/api/guests/csv")
    def upload_csv():
        file = request.files.get("file")
        if file is None:
            raise BadRequest("CSV ファイルを選択してください")
        try:
            guests = parse_csv(file.read())
        except ValueError as err:
            raise BadRequest(str(err)) from err
        return jsonify(guests=[asdict(g) for g in guests])

    @app.get("/api/guests/sample.csv")
    def download_sample():
        resp = send_file(io.BytesIO(sample_csv()), mimetype="text/csv", as_attachment=True,
                         download_name="guests_sample.csv")
        resp.headers["Content-Disposition"] = (
            f"attachment; filename=guests_sample.csv; filename*=UTF-8''{quote('ゲスト名簿_サンプル.csv')}")
        return resp

    return app


app = create_app()

if __name__ == "__main__":
    app.run(debug=True)
