import init, * as wasm from "./pkg/wedding_invitation.js";

const FONTS = {
  ShipporiMincho: "fonts/ShipporiMincho-Regular.ttf",
  GreatVibes: "fonts/GreatVibes-Regular.ttf",
};
const GUEST_FIELDS = ["name", "postal_code", "address1", "address2", "joint_name", "honorific"];
const STORAGE_KEY = "wedding-invitation-form";
const FILE_NAMES = {
  zip: "wedding_invitation_print_data.zip",
  xlsx: "wedding_invitation.xlsx",
  access: "access_map.pdf",
  guestsXlsx: "guests_onew.xlsx",
  guestsCsv: "guests_onew.csv",
  card: "invitation_cards.pdf",
  envelope: "envelopes.pdf",
};

const $ = (id) => document.getElementById(id);
const form = $("form");
const rows = $("guest-rows");
const preview = $("preview");
const errorBox = $("error");
const guestSelect = $("preview-guest");

let defaults = {};
let maxGuests = 500;
let previewDoc = "card";
let previewTimer = null;

// 案内図
let accessInfo = { access: { title: "", routes: [], steps: [] }, modes: [], max_routes: 8, max_steps: 6 };
let mapPhotoId = "";
let cardPhotoId = "";
const photoUrls = new Map(); // 写真 ID -> 画面表示用の data URL
const STEP_EDGE = 1200; // 道順の写真の長辺(px)
const PREVIEW_EDGE = 640; // プレビュー用の長辺(px)

// ------------------------------------------------------------ 起動
async function boot() {
  const [, mincho, script] = await Promise.all([
    init(),
    fetch(FONTS.ShipporiMincho).then((r) => r.arrayBuffer()),
    fetch(FONTS.GreatVibes).then((r) => r.arrayBuffer()),
  ]);
  // PDF 用(wasm に登録)とプレビュー用(Web フォントとして登録)
  wasm.initFonts(new Uint8Array(mincho), new Uint8Array(script));
  for (const [family, data] of [["ShipporiMincho", mincho], ["GreatVibes", script]]) {
    const face = new FontFace(family, data.slice(0));
    await face.load();
    document.fonts.add(face);
  }
  defaults = JSON.parse(wasm.defaultEventJson());
  maxGuests = wasm.maxGuests();
  accessInfo = JSON.parse(wasm.accessDefaultsJson());
  renderDesigns(JSON.parse(wasm.designsJson()));
  renderLayouts(accessInfo.card_layouts || []);
  const saved = load();
  await restorePhotos(saved);
  fill(saved);
  bindEvents();
  refreshGuestSelect(guests());
  updatePreview();
  $("boot").classList.add("hidden");
}

// レイアウトの見本(ページの中の文字の並び)
const LAYOUT_ICONS = {
  horizontal: '<rect x="1" y="1" width="32" height="46" rx="2" fill="none" stroke="currentColor" stroke-width="1.5"/>' +
    [12, 17, 21, 25, 29, 35, 39].map((y) => `<line x1="9" y1="${y}" x2="25" y2="${y}" stroke="currentColor"/>`).join(""),
  vertical: '<rect x="1" y="1" width="32" height="46" rx="2" fill="none" stroke="currentColor" stroke-width="1.5"/>' +
    [26, 22, 18, 14, 10].map((x) => `<line x1="${x}" y1="8" x2="${x}" y2="28" stroke="currentColor"/>`).join("") +
    [35, 39].map((y) => `<line x1="9" y1="${y}" x2="25" y2="${y}" stroke="currentColor"/>`).join(""),
  photo: '<rect x="1" y="1" width="32" height="46" rx="2" fill="none" stroke="currentColor" stroke-width="1.5"/>' +
    '<rect x="5" y="5" width="24" height="13" fill="currentColor" opacity="0.45"/>' +
    [24, 28, 32, 36, 40].map((y) => `<line x1="9" y1="${y}" x2="25" y2="${y}" stroke="currentColor"/>`).join(""),
};

function renderLayouts(layouts) {
  const box = $("layouts");
  for (const [i, l] of layouts.entries()) {
    const label = document.createElement("label");
    label.className = "layout-card";
    label.innerHTML = `<input type="radio" name="card_layout"><span class="body">
      <svg viewBox="0 0 34 48" aria-hidden="true">${LAYOUT_ICONS[l.key] || ""}</svg><span></span></span>`;
    const input = label.querySelector("input");
    input.value = l.key;
    input.checked = i === 0;
    label.querySelector(".body > span").textContent = l.label;
    box.appendChild(label);
  }
}

function cardLayout() {
  return form.querySelector("input[name=card_layout]:checked")?.value || "horizontal";
}

function setCardPhoto(id) {
  cardPhotoId = id || "";
  setThumb($("card-photo-thumb"), cardPhotoId, "二人の写真");
  $("card-photo-remove").hidden = !cardPhotoId;
  $("card-photo-slot").hidden = cardLayout() !== "photo";
}

function renderDesigns(designs) {
  const box = $("designs");
  for (const [i, d] of designs.entries()) {
    const label = document.createElement("label");
    label.className = "design";
    label.innerHTML = `
      <input type="radio" name="design">
      <span class="design-body">
        <span class="swatches"></span>
        <strong></strong>
        <small></small>
      </span>`;
    const input = label.querySelector("input");
    input.value = d.key;
    input.checked = i === 0;
    label.querySelector("strong").textContent = d.name;
    label.querySelector("small").textContent = d.description;
    for (const color of d.swatches) {
      const dot = document.createElement("span");
      dot.style.background = color;
      label.querySelector(".swatches").appendChild(dot);
    }
    box.appendChild(label);
  }
  // 案内図だけ別のデザインにする選択肢(先頭の「招待状と同じ」は HTML 側)
  const accessBox = $("access-designs");
  for (const d of designs) {
    const label = document.createElement("label");
    label.className = "orient";
    label.innerHTML = `<input type="radio" name="access_design"><span><span class="swatches"></span></span>`;
    label.querySelector("input").value = d.key;
    const body = label.querySelector("span");
    for (const color of d.swatches.slice(0, 3)) {
      const dot = document.createElement("span");
      dot.style.background = color;
      body.querySelector(".swatches").appendChild(dot);
    }
    body.append(d.name);
    accessBox.appendChild(label);
  }
}

// ------------------------------------------------------------ 状態の保存
function load() {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY)) || null;
  } catch {
    return null;
  }
}

function save(order) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(order));
  } catch {
    /* 保存できなくても動作には影響しない */
  }
}

// ------------------------------------------------------------ ゲスト表
function addGuest(guest = {}) {
  const tr = document.createElement("tr");
  for (const key of GUEST_FIELDS) {
    const td = document.createElement("td");
    const input = document.createElement("input");
    input.dataset.field = key;
    input.value = guest[key] ?? (key === "honorific" ? "様" : "");
    if (key === "name") input.placeholder = "山田 太郎";
    if (key === "postal_code") input.placeholder = "123-4567";
    if (key === "honorific") input.placeholder = "様";
    td.appendChild(input);
    tr.appendChild(td);
  }
  // 印刷に使わない One-W の列(肩書き・ふりがな・出欠など)は書き出し用に持っておく
  if (guest.extra && Object.keys(guest.extra).length) {
    tr.dataset.extra = JSON.stringify(guest.extra);
    if (guest.extra["肩書き"]) tr.title = guest.extra["肩書き"];
  }
  const td = document.createElement("td");
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "remove";
  btn.title = "削除";
  btn.textContent = "×";
  btn.addEventListener("click", () => {
    tr.remove();
    if (!rows.children.length) addGuest();
    changed();
  });
  td.appendChild(btn);
  tr.appendChild(td);
  rows.appendChild(tr);
  return tr;
}

function guests() {
  return [...rows.children]
    .map((tr) => {
      const g = Object.fromEntries(
        [...tr.querySelectorAll("input")].map((i) => [i.dataset.field, i.value.trim()]));
      if (tr.dataset.extra) g.extra = JSON.parse(tr.dataset.extra);
      return g;
    })
    .filter((g) => g.name);
}

function refreshGuestSelect(list) {
  const current = Number(guestSelect.value || 0);
  guestSelect.innerHTML = "";
  list.forEach((g, i) => {
    const opt = document.createElement("option");
    opt.value = i;
    opt.textContent = `${i + 1}. ${g.name} ${g.honorific || "様"}`;
    guestSelect.appendChild(opt);
  });
  guestSelect.value = String(Math.min(current, Math.max(list.length - 1, 0)));
  $("guest-count").textContent = `${list.length} 名`;
}

// ------------------------------------------------------------ 写真
// 縮小した JPEG を IndexedDB に保存し、再読み込みしても写真が残るようにする(保存できなくても動作する)
const DB_NAME = "wedding-invitation-photos";

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore("photos");
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function dbRun(mode, fn) {
  try {
    const db = await openDb();
    return await new Promise((resolve, reject) => {
      const tx = db.transaction("photos", mode);
      const req = fn(tx.objectStore("photos"));
      tx.oncomplete = () => resolve(req?.result);
      tx.onerror = () => reject(tx.error);
    });
  } catch {
    return undefined;
  }
}

const newPhotoId = () => `p${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`;

function canvasFor(bitmap, maxEdge) {
  const scale = Math.min(1, maxEdge / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  const ctx = canvas.getContext("2d");
  ctx.fillStyle = "#ffffff"; // 透明部分は白にする(印刷で透明は使わない)
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  return canvas;
}

// 画像を縮小・デコードして wasm に登録する(CMYK への変換は wasm 側)
async function registerPhoto(id, blob, maxEdge, persist) {
  const bitmap = await createImageBitmap(blob, { imageOrientation: "from-image" });
  const canvas = canvasFor(bitmap, maxEdge);
  const rgba = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
  const previewUrl = canvasFor(bitmap, PREVIEW_EDGE).toDataURL("image/jpeg", 0.82);
  bitmap.close?.();
  wasm.addPhoto(id, canvas.width, canvas.height, new Uint8Array(rgba.buffer), previewUrl);
  photoUrls.set(id, previewUrl);
  if (persist) {
    const stored = await new Promise((r) => canvas.toBlob(r, "image/jpeg", 0.92));
    await dbRun("readwrite", (s) => s.put({ blob: stored, maxEdge }, id));
  }
}

function forgetPhoto(id) {
  if (!id) return;
  wasm.removePhoto(id);
  photoUrls.delete(id);
  dbRun("readwrite", (s) => s.delete(id));
}

async function restorePhotos(state) {
  const access = state?.access;
  const ids = [state?.card?.photo, access?.map_photo, ...(access?.steps || []).map((s) => s.photo)].filter(Boolean);
  for (const id of ids) {
    const rec = await dbRun("readonly", (s) => s.get(id));
    if (!rec?.blob) continue;
    try {
      await registerPhoto(id, rec.blob, rec.maxEdge || STEP_EDGE, false);
    } catch {
      /* 壊れた写真は読み飛ばす */
    }
  }
}

function setThumb(box, id, emptyText) {
  box.innerHTML = "";
  const url = id && photoUrls.get(id);
  box.classList.toggle("filled", Boolean(url));
  if (url) {
    const img = document.createElement("img");
    img.src = url;
    img.alt = "";
    box.appendChild(img);
  } else {
    const span = document.createElement("span");
    span.textContent = id ? "写真を選び直してください" : emptyText;
    box.appendChild(span);
  }
}

// ------------------------------------------------------------ 案内図
function setMapPhoto(id) {
  mapPhotoId = id || "";
  setThumb($("map-thumb"), mapPhotoId, "地図の画像(スクリーンショットなど)");
  $("map-remove").hidden = !mapPhotoId;
}

function addRoute(route = {}) {
  const box = $("routes");
  if (box.children.length >= accessInfo.max_routes) return null;
  const row = document.createElement("div");
  row.className = "route";
  const select = document.createElement("select");
  for (const m of accessInfo.modes) {
    const opt = document.createElement("option");
    opt.value = m.key;
    opt.textContent = m.label;
    select.appendChild(opt);
  }
  select.value = route.mode || "train";
  const input = document.createElement("input");
  input.value = route.text || "";
  input.placeholder = "例: JR 〇〇駅 中央口より徒歩 5 分";
  const remove = document.createElement("button");
  remove.type = "button";
  remove.className = "remove";
  remove.title = "削除";
  remove.textContent = "×";
  remove.addEventListener("click", () => {
    row.remove();
    changed();
  });
  row.append(select, input, remove);
  box.appendChild(row);
  return row;
}

function routes() {
  return [...$("routes").children]
    .map((row) => ({ mode: row.querySelector("select").value, text: row.querySelector("input").value.trim() }))
    .filter((r) => r.text);
}

function renumberSteps() {
  [...$("steps").children].forEach((card, i, all) => {
    card.querySelector(".num").textContent = i + 1;
    card.querySelector(".up").disabled = i === 0;
    card.querySelector(".down").disabled = i === all.length - 1;
  });
}

function addStep(step = {}) {
  const box = $("steps");
  if (box.children.length >= accessInfo.max_steps) return null;
  const card = document.createElement("div");
  card.className = "step-card";
  card.dataset.photo = step.photo || "";
  card.innerHTML = `
    <div class="bar"><span class="num"></span><span class="spacer"></span>
      <button type="button" class="up" title="前へ">↑</button>
      <button type="button" class="down" title="後ろへ">↓</button>
      <button type="button" class="remove" title="削除">×</button></div>
    <label class="thumb" title="クリックで写真を変更"><input type="file" accept="image/*" hidden></label>
    <textarea rows="2" placeholder="例: 改札を出て右へ進みます"></textarea>`;
  const thumb = card.querySelector(".thumb");
  const fileInput = thumb.querySelector("input");
  const showThumb = () => {
    setThumb(thumb, card.dataset.photo, "クリックして写真を選ぶ");
    thumb.prepend(fileInput);
  };
  showThumb();
  card.querySelector("textarea").value = step.caption || "";
  fileInput.addEventListener("change", async (e) => {
    const file = e.target.files[0];
    e.target.value = "";
    if (!file) return;
    try {
      const id = newPhotoId();
      await registerPhoto(id, file, STEP_EDGE, true);
      forgetPhoto(card.dataset.photo);
      card.dataset.photo = id;
      showThumb();
      changed();
    } catch (err) {
      showError(`写真を読み込めませんでした: ${err.message || err}`);
    }
  });
  card.querySelector(".up").addEventListener("click", () => {
    if (card.previousElementSibling) box.insertBefore(card, card.previousElementSibling);
    renumberSteps();
    changed();
  });
  card.querySelector(".down").addEventListener("click", () => {
    if (card.nextElementSibling) box.insertBefore(card.nextElementSibling, card);
    renumberSteps();
    changed();
  });
  card.querySelector(".remove").addEventListener("click", () => {
    forgetPhoto(card.dataset.photo);
    card.remove();
    renumberSteps();
    changed();
  });
  box.appendChild(card);
  renumberSteps();
  return card;
}

function steps() {
  return [...$("steps").children]
    .map((card) => ({ photo: card.dataset.photo || "", caption: card.querySelector("textarea").value.trim() }))
    .filter((s) => s.photo || s.caption);
}

function access() {
  return {
    title: form.elements.access_title.value.trim() || "ご案内図",
    map_photo: mapPhotoId,
    routes: routes(),
    steps: steps(),
    note: form.elements.access_note.value.trim(),
    steps_title: form.elements.steps_title.value.trim(),
    orientation: form.querySelector("input[name=access_orientation]:checked")?.value || "portrait",
    design: form.querySelector("input[name=access_design]:checked")?.value || "",
  };
}

function fillAccess(a) {
  const value = { ...accessInfo.access, ...(a || {}) };
  form.elements.access_title.value = value.title || "";
  form.elements.access_note.value = value.note || "";
  form.elements.steps_title.value = value.steps_title || "";
  const orientation = value.orientation === "landscape" ? "landscape" : "portrait";
  form.querySelector(`input[name=access_orientation][value=${orientation}]`).checked = true;
  const design = form.querySelector(`input[name=access_design][value="${CSS.escape(value.design || "")}"]`);
  (design || form.querySelector("input[name=access_design]")).checked = true;
  setMapPhoto(value.map_photo);
  $("routes").innerHTML = "";
  (value.routes || []).forEach((r) => addRoute(r));
  $("steps").innerHTML = "";
  (value.steps || []).forEach((s) => addStep(s));
}

async function addStepPhotos(files) {
  for (const file of files) {
    if ($("steps").children.length >= accessInfo.max_steps) {
      showError(`道順の写真は ${accessInfo.max_steps} 枚までです`);
      break;
    }
    try {
      const id = newPhotoId();
      await registerPhoto(id, file, STEP_EDGE, true);
      addStep({ photo: id });
    } catch (err) {
      showError(`写真を読み込めませんでした: ${err.message || err}`);
    }
  }
  changed();
}

// ------------------------------------------------------------ 入力値
function order() {
  const fd = new FormData(form);
  const event = {};
  for (const key of Object.keys(defaults)) event[key] = fd.get(key) ?? "";
  return {
    design: fd.get("design"),
    event,
    guests: guests(),
    options: {
      crop_marks: form.elements.crop_marks.checked,
      envelope_back: form.elements.envelope_back.checked,
    },
    access: access(),
    card: { layout: cardLayout(), photo: cardPhotoId },
  };
}

function showNotice(text, warn = false) {
  const box = $("notice");
  box.textContent = text;
  box.classList.toggle("warn", warn);
}

function fill(state) {
  const event = { ...defaults, ...(state?.event || {}) };
  for (const [key, value] of Object.entries(event)) {
    if (form.elements[key]) form.elements[key].value = value;
  }
  if (state?.design) {
    const radio = form.querySelector(`input[name=design][value="${CSS.escape(state.design)}"]`);
    if (radio) radio.checked = true;
  }
  if (state?.options) {
    form.elements.crop_marks.checked = state.options.crop_marks !== false;
    form.elements.envelope_back.checked = state.options.envelope_back !== false;
  }
  const list = state?.guests?.length ? state.guests : [
    { name: "山田 太郎", postal_code: "150-0001", address1: "東京都渋谷区神宮前1-23-45",
      address2: "グランドメゾン原宿 302号室", joint_name: "花子", honorific: "様" },
  ];
  rows.innerHTML = "";
  list.forEach((g) => addGuest(g));
  fillAccess(state?.access);
  const layout = state?.card?.layout;
  const radio = layout && form.querySelector(`input[name=card_layout][value="${CSS.escape(layout)}"]`);
  if (radio) radio.checked = true;
  setCardPhoto(state?.card?.photo);
}

// ------------------------------------------------------------ プレビュー・出力
function showError(err) {
  errorBox.textContent = err ? (err.message || String(err)) : "";
}

function updatePreview() {
  const o = order();
  const needsGuests = previewDoc !== "access";
  guestSelect.hidden = !needsGuests;
  if (needsGuests && !o.guests.length) {
    showError("ゲストを 1 名以上入力してください");
    return;
  }
  try {
    const svg = wasm.renderPreview(JSON.stringify(o), previewDoc, Number(guestSelect.value || 0));
    preview.innerHTML = needsGuests ? svg : `<div class="pages">${svg}</div>`;
    showError(null);
  } catch (err) {
    showError(err);
  }
}

function changed() {
  const o = order();
  refreshGuestSelect(o.guests);
  save(o);
  clearTimeout(previewTimer);
  previewTimer = setTimeout(updatePreview, 250);
}

function download(bytes, filename, type) {
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
}

async function generate(doc, button) {
  const o = order();
  if (doc !== "access" && !o.guests.length) {
    showError("ゲストを 1 名以上入力してください");
    return;
  }
  const label = button.textContent;
  button.disabled = true;
  button.textContent = "作成中…";
  showError(null);
  // ボタンの表示を更新してから重い処理を始める
  await new Promise((r) => requestAnimationFrame(() => setTimeout(r)));
  try {
    const json = JSON.stringify(o);
    if (doc === "zip") {
      download(wasm.renderZip(json), FILE_NAMES.zip, "application/zip");
    } else {
      download(wasm.renderPdf(json, doc), FILE_NAMES[doc], "application/pdf");
    }
  } catch (err) {
    showError(err);
  } finally {
    button.disabled = false;
    button.textContent = label;
  }
}

function replaceGuests(list, label) {
  if (!list.length) throw new Error(`${label}にゲストが見つかりませんでした`);
  const replace = guests().length === 0 ||
    confirm(`${list.length} 名を読み込みました。現在のゲスト一覧を置き換えますか？\n(キャンセルで末尾に追加)`);
  if (replace) rows.innerHTML = "";
  list.slice(0, maxGuests - (replace ? 0 : rows.children.length)).forEach((g) => addGuest(g));
  changed();
}

// 名簿だけを読み込む(One-W の CSV / xlsx、簡易形式の CSV)
async function importGuests(file) {
  showError(null);
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (/\.xlsx$/i.test(file.name)) {
      const { order: imported, warnings } = JSON.parse(wasm.importXlsx(bytes));
      replaceGuests(imported.guests, "Excel ");
      if (warnings.length) showNotice(warnings.join("\n"), true);
    } else {
      replaceGuests(JSON.parse(wasm.parseGuestsCsv(bytes)), "CSV ");
    }
  } catch (err) {
    showError(err);
  }
}

function exportGuests(kind) {
  try {
    const json = JSON.stringify(order());
    if (kind === "xlsx") {
      download(wasm.exportGuestsXlsx(json), FILE_NAMES.guestsXlsx,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
    } else {
      download(wasm.exportGuestsCsv(json), FILE_NAMES.guestsCsv, "text/csv");
    }
  } catch (err) {
    showError(err);
  }
}

function exportXlsx() {
  try {
    const bytes = wasm.exportXlsx(JSON.stringify(order()));
    download(bytes, FILE_NAMES.xlsx, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
  } catch (err) {
    showError(err);
  }
}

async function importXlsx(file) {
  showError(null);
  showNotice("");
  try {
    const { order: imported, has_settings: hasSettings, warnings } =
      JSON.parse(wasm.importXlsx(new Uint8Array(await file.arrayBuffer())));
    if (!hasSettings) {
      // One-W の出席者名簿など名簿だけのファイル: 挙式情報やデザインはそのまま
      if (!confirm(`名簿だけのファイルです。ゲスト一覧を ${imported.guests.length} 名で置き換えます。よろしいですか？`)) return;
      rows.innerHTML = "";
      (imported.guests.length ? imported.guests : [{}]).forEach((g) => addGuest(g));
      changed();
      const summary = `「${file.name}」からゲスト ${imported.guests.length} 名を読み込みました(挙式情報・デザインは変更していません)`;
      showNotice(warnings.length ? `${summary}\n${warnings.join("\n")}` : summary, warnings.length > 0);
      return;
    }
    if (!confirm("現在の入力内容を Excel の内容で置き換えます。よろしいですか？")) return;
    // 写真は Excel に入らないので、今の地図と道順の写真はそのまま残す
    imported.access = { ...imported.access, map_photo: mapPhotoId, steps: steps() };
    imported.card = { ...imported.card, photo: cardPhotoId };
    fill(imported);
    if (!imported.guests.length) {
      rows.innerHTML = "";
      addGuest();
    }
    changed();
    const summary = `「${file.name}」を読み込みました(ゲスト ${imported.guests.length} 名)`;
    showNotice(warnings.length ? `${summary}\n${warnings.join("\n")}` : summary, warnings.length > 0);
  } catch (err) {
    showError(err);
  }
}

function bindEvents() {
  form.addEventListener("input", changed);
  form.addEventListener("change", changed);
  $("add-guest").addEventListener("click", () => {
    if (rows.children.length >= maxGuests) return;
    addGuest().querySelector("input").focus();
  });
  $("csv").addEventListener("change", (e) => {
    if (e.target.files[0]) importGuests(e.target.files[0]);
    e.target.value = "";
  });
  $("export-xlsx").addEventListener("click", exportXlsx);
  $("add-route").addEventListener("click", () => {
    addRoute()?.querySelector("input").focus();
  });
  $("map-file").addEventListener("change", async (e) => {
    const file = e.target.files[0];
    e.target.value = "";
    if (!file) return;
    try {
      const id = newPhotoId();
      await registerPhoto(id, file, wasm.maxPhotoEdge(), true);
      forgetPhoto(mapPhotoId);
      setMapPhoto(id);
      changed();
    } catch (err) {
      showError(`画像を読み込めませんでした: ${err.message || err}`);
    }
  });
  $("layouts").addEventListener("change", () => setCardPhoto(cardPhotoId));
  $("card-photo-file").addEventListener("change", async (e) => {
    const file = e.target.files[0];
    e.target.value = "";
    if (!file) return;
    try {
      const id = newPhotoId();
      await registerPhoto(id, file, wasm.maxPhotoEdge(), true);
      forgetPhoto(cardPhotoId);
      setCardPhoto(id);
      changed();
    } catch (err) {
      showError(`写真を読み込めませんでした: ${err.message || err}`);
    }
  });
  $("card-photo-remove").addEventListener("click", () => {
    forgetPhoto(cardPhotoId);
    setCardPhoto("");
    changed();
  });
  $("map-remove").addEventListener("click", () => {
    forgetPhoto(mapPhotoId);
    setMapPhoto("");
    changed();
  });
  $("step-files").addEventListener("change", (e) => {
    const files = [...e.target.files];
    e.target.value = "";
    addStepPhotos(files);
  });
  $("export-guests-xlsx").addEventListener("click", () => exportGuests("xlsx"));
  $("export-guests-csv").addEventListener("click", () => exportGuests("csv"));
  $("xlsx").addEventListener("change", (e) => {
    if (e.target.files[0]) importXlsx(e.target.files[0]);
    e.target.value = "";
  });
  $("sample-csv").addEventListener("click", (e) => {
    e.preventDefault();
    download(wasm.sampleCsv(), "guests_onew_sample.csv", "text/csv");
  });
  document.querySelectorAll("[data-generate]").forEach((btn) =>
    btn.addEventListener("click", () => generate(btn.dataset.generate, btn)));
  document.querySelectorAll(".tab").forEach((tab) =>
    tab.addEventListener("click", () => {
      document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t === tab));
      previewDoc = tab.dataset.doc;
      updatePreview();
    }));
  guestSelect.addEventListener("change", updatePreview);
}

boot().catch((err) => {
  console.error(err);
  const box = $("boot");
  box.classList.add("failed");
  box.textContent = `起動できませんでした: ${err.message || err}`;
});
