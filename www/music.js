import init, * as wasm from "./pkg/wedding_invitation.js";

const STORAGE_KEY = "wedding-music";
const COLORS = {
  "選定中": "#cbbfa9",
  "決定": "#6f8fc6",
  "発注済み": "#c99a36",
  "納品済み": "#5c9a6a",
};

const $ = (id) => document.getElementById(id);
let config = { statuses: [], max_songs: 100, max_candidates: 10 };
let state = { version: 1, songs: [] };
let filter = "";

// ------------------------------------------------------------ 保存
function save() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    /* 保存できなくても動作には影響しない */
  }
}

function loadSaved() {
  try {
    const text = localStorage.getItem(STORAGE_KEY);
    return text ? JSON.parse(wasm.musicNormalize(text)).list : null;
  } catch {
    return null;
  }
}

function showError(text) {
  $("error").textContent = text || "";
}

function showNotice(text, warn = false) {
  $("notice").textContent = text || "";
  $("notice").classList.toggle("warn", warn);
}

// ------------------------------------------------------------ 集計
function renderSummary() {
  const s = JSON.parse(wasm.musicSummary(JSON.stringify(state)));
  const done = s.by_status.find((c) => c.label === "納品済み")?.count || 0;
  $("done-count").textContent = done;
  $("done-of").textContent = `/ ${s.total} シーン 納品済み(${s.done_percent}%)`;

  const stack = $("stack");
  stack.innerHTML = "";
  stack.setAttribute("aria-label", s.by_status.map((c) => `${c.label} ${c.count}`).join("、"));
  const legend = $("legend");
  legend.innerHTML = "";
  for (const c of s.by_status) {
    const seg = document.createElement("span");
    seg.style.width = s.total ? `${(c.count / s.total) * 100}%` : "0";
    seg.style.background = COLORS[c.label];
    seg.title = `${c.label} ${c.count} 件`;
    stack.appendChild(seg);
    const li = document.createElement("li");
    li.innerHTML = `<i></i><span></span><b></b>`;
    li.querySelector("i").style.background = COLORS[c.label];
    li.querySelector("span").textContent = c.label;
    li.querySelector("b").textContent = `${c.count} 件`;
    legend.appendChild(li);
  }
  $("undecided").textContent = s.undecided.length ? s.undecided.join("、") : "なし";
  $("missing-vendor").textContent = s.missing_vendor.length ? s.missing_vendor.join("、") : "なし";
  $("missing-title").textContent = s.missing_title.length ? s.missing_title.join("、") : "なし";

  const vendors = $("vendors");
  vendors.innerHTML = "";
  const max = Math.max(1, ...s.by_vendor.map((v) => v.count));
  for (const v of s.by_vendor) {
    const li = document.createElement("li");
    li.innerHTML = `<div class="row"><span></span><b></b></div><div class="bar"><span></span></div>`;
    li.querySelector(".row span").textContent = v.label;
    li.querySelector(".row b").textContent = `${v.count} 件`;
    li.querySelector(".bar span").style.width = `${(v.count / max) * 100}%`;
    if (v.label === "未定") li.querySelector(".bar span").style.background = COLORS["選定中"];
    vendors.appendChild(li);
  }
  if (!s.by_vendor.length) vendors.innerHTML = "<li>—</li>";
}

function changed() {
  save();
  renderSummary();
}

// ------------------------------------------------------------ 表
function renderFilters() {
  const box = $("filters");
  box.innerHTML = "";
  for (const value of ["", ...config.statuses]) {
    const btn = document.createElement("button");
    btn.type = "button";
    const count = value ? state.songs.filter((s) => s.status === value).length : state.songs.length;
    btn.textContent = `${value || "すべて"} ${count}`;
    btn.setAttribute("aria-pressed", String(filter === value));
    btn.addEventListener("click", () => {
      filter = value;
      render();
    });
    box.appendChild(btn);
  }
}

function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  Object.assign(node, props);
  node.append(...children);
  return node;
}

function titlesCell(song) {
  const td = el("td", { className: "titles" });
  if (song.status !== "選定中") {
    const input = el("input", { value: song.titles[0] || "", placeholder: "曲名(アーティスト)" });
    input.addEventListener("input", () => {
      song.titles = input.value.trim() ? [input.value.trim()] : [];
      changed();
    });
    td.append(input);
    return td;
  }
  // 選定中: 候補を複数持ち、「決定」で 1 曲に絞る
  const chips = el("div", { className: "chips" });
  song.titles.forEach((title, i) => {
    const pick = el("button", { type: "button", className: "pick", textContent: "決定", title: "この曲に決定" });
    pick.addEventListener("click", () => {
      song.titles = [title];
      song.status = "決定";
      showError("");
      render();
    });
    const remove = el("button", { type: "button", textContent: "×", title: "候補から外す" });
    remove.addEventListener("click", () => {
      song.titles.splice(i, 1);
      render();
    });
    chips.append(el("span", { className: "chip" }, el("span", { textContent: title }), pick, remove));
  });
  const input = el("input", { placeholder: song.titles.length ? "候補を追加(Enter)" : "候補の曲名を入力(Enter で追加)" });
  const add = () => {
    const title = input.value.trim();
    if (!title) return;
    if (song.titles.length >= config.max_candidates) {
      showError(`候補は ${config.max_candidates} 曲までです`);
      return;
    }
    if (!song.titles.includes(title)) song.titles.push(title);
    render();
    // 続けて入力できるよう、同じ行の入力欄に戻る
    document.querySelector(`tr[data-index="${state.songs.indexOf(song)}"] td.titles input`)?.focus();
  };
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.isComposing) {
      e.preventDefault();
      add();
    }
  });
  const addBtn = el("button", { type: "button", textContent: "追加" });
  addBtn.addEventListener("click", add);
  td.append(chips, el("div", { className: "add-row" }, input, addBtn));
  if (song.titles.length > 1) {
    td.append(el("small", { className: "hint", textContent: "候補の「決定」で 1 曲に絞ります" }));
  }
  return td;
}

function statusCell(song) {
  const select = el("select", { className: "status-select" });
  for (const s of config.statuses) select.append(el("option", { value: s, textContent: s }));
  select.value = song.status;
  select.style.borderColor = COLORS[song.status];
  select.style.color = song.status === "選定中" ? "" : COLORS[song.status];
  select.addEventListener("change", () => {
    if (select.value !== "選定中" && song.titles.length > 1) {
      showError(`「${song.scene || "このシーン"}」は候補が ${song.titles.length} 曲あります。候補の「決定」で 1 曲に絞ってください`);
      select.value = song.status;
      return;
    }
    showError("");
    song.status = select.value;
    render();
  });
  return el("td", { className: "status" }, select);
}

function render() {
  const body = $("songs");
  body.innerHTML = "";
  state.songs.forEach((song, i) => {
    const tr = el("tr");
    tr.dataset.index = i;
    if (filter && song.status !== filter) tr.classList.add("hidden");
    const scene = el("input", { value: song.scene, placeholder: "例: 新郎新婦入場" });
    scene.addEventListener("input", () => {
      song.scene = scene.value.trim();
      changed();
    });
    const vendor = el("input", { value: song.vendor, placeholder: "例: 会場" });
    vendor.setAttribute("list", "vendor-list");
    vendor.addEventListener("input", () => {
      song.vendor = vendor.value.trim();
      changed();
    });
    const move = (delta) => {
      const j = i + delta;
      if (j < 0 || j >= state.songs.length) return;
      [state.songs[i], state.songs[j]] = [state.songs[j], state.songs[i]];
      render();
    };
    const up = el("button", { type: "button", textContent: "↑", title: "上へ", disabled: i === 0 });
    up.addEventListener("click", () => move(-1));
    const down = el("button", { type: "button", textContent: "↓", title: "下へ", disabled: i === state.songs.length - 1 });
    down.addEventListener("click", () => move(1));
    const remove = el("button", { type: "button", className: "remove", textContent: "×", title: "削除" });
    remove.addEventListener("click", () => {
      if (song.titles.length && !confirm(`「${song.scene || "このシーン"}」を削除しますか？`)) return;
      state.songs.splice(i, 1);
      render();
    });
    tr.append(
      el("td", { className: "num", textContent: i + 1 }),
      el("td", { className: "scene" }, scene),
      titlesCell(song),
      statusCell(song),
      el("td", { className: "vendor" }, vendor),
      el("td", { className: "ops" }, up, down, remove),
    );
    body.appendChild(tr);
  });
  renderFilters();
  changed();
}

// ------------------------------------------------------------ JSON
function download(text, filename) {
  const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
  const a = el("a", { href: url, download: filename });
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
}

async function importJson(file) {
  showError("");
  showNotice("");
  try {
    const { list, warnings } = JSON.parse(wasm.musicNormalize(await file.text()));
    if (!confirm(`${list.songs.length} 件のシーンを読み込みます。今の内容を置き換えますか？`)) return;
    state = list;
    filter = "";
    render();
    const summary = `「${file.name}」から ${list.songs.length} 件を読み込みました`;
    showNotice(warnings.length ? `${summary}\n${warnings.join("\n")}` : summary, warnings.length > 0);
  } catch (err) {
    showError(err.message || String(err));
  }
}

function bind() {
  $("json-file").addEventListener("change", (e) => {
    if (e.target.files[0]) importJson(e.target.files[0]);
    e.target.value = "";
  });
  $("export-json").addEventListener("click", () => {
    download(wasm.musicToJson(JSON.stringify(state)), "music.json");
  });
  $("add-song").addEventListener("click", () => {
    if (state.songs.length >= config.max_songs) {
      showError(`シーンは ${config.max_songs} 件までです`);
      return;
    }
    state.songs.push({ scene: "", titles: [], status: "選定中", vendor: "" });
    filter = "";
    render();
    document.querySelector("#songs tr:last-child td.scene input")?.focus();
  });
  $("reset").addEventListener("click", () => {
    if (!confirm("楽曲の一覧を初期状態に戻します。よろしいですか？")) return;
    state = JSON.parse(wasm.musicDefaultJson());
    filter = "";
    render();
  });
}

async function boot() {
  await init();
  config = JSON.parse(wasm.musicConfigJson());
  state = loadSaved() || JSON.parse(wasm.musicDefaultJson());
  bind();
  render();
  $("boot").classList.add("hidden");
}

boot().catch((err) => {
  console.error(err);
  const box = $("boot");
  box.classList.add("failed");
  box.textContent = `起動できませんでした: ${err.message || err}`;
});
