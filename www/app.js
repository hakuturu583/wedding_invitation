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
  renderDesigns(JSON.parse(wasm.designsJson()));
  fill(load());
  bindEvents();
  refreshGuestSelect(guests());
  updatePreview();
  $("boot").classList.add("hidden");
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
    td.appendChild(input);
    tr.appendChild(td);
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
    .map((tr) => Object.fromEntries(
      [...tr.querySelectorAll("input")].map((i) => [i.dataset.field, i.value.trim()])))
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
}

// ------------------------------------------------------------ プレビュー・出力
function showError(err) {
  errorBox.textContent = err ? (err.message || String(err)) : "";
}

function updatePreview() {
  const o = order();
  if (!o.guests.length) {
    showError("ゲストを 1 名以上入力してください");
    return;
  }
  try {
    preview.innerHTML = wasm.renderPreview(JSON.stringify(o), previewDoc, Number(guestSelect.value || 0));
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
  if (!o.guests.length) {
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

async function importCsv(file) {
  showError(null);
  try {
    const list = JSON.parse(wasm.parseGuestsCsv(new Uint8Array(await file.arrayBuffer())));
    if (!list.length) throw new Error("CSV にゲストが見つかりませんでした");
    const replace = guests().length === 0 ||
      confirm(`${list.length} 名を読み込みました。現在のゲスト一覧を置き換えますか？\n(キャンセルで末尾に追加)`);
    if (replace) rows.innerHTML = "";
    list.slice(0, maxGuests - (replace ? 0 : rows.children.length)).forEach((g) => addGuest(g));
    changed();
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
    const { order: imported, warnings } = JSON.parse(wasm.importXlsx(new Uint8Array(await file.arrayBuffer())));
    if (!confirm("現在の入力内容を Excel の内容で置き換えます。よろしいですか？")) return;
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
    if (e.target.files[0]) importCsv(e.target.files[0]);
    e.target.value = "";
  });
  $("export-xlsx").addEventListener("click", exportXlsx);
  $("xlsx").addEventListener("change", (e) => {
    if (e.target.files[0]) importXlsx(e.target.files[0]);
    e.target.value = "";
  });
  $("sample-csv").addEventListener("click", (e) => {
    e.preventDefault();
    download(wasm.sampleCsv(), "guests_sample.csv", "text/csv");
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
