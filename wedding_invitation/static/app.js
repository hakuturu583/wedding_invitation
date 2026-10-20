(() => {
  "use strict";

  const form = document.getElementById("form");
  const rows = document.getElementById("guest-rows");
  const image = document.getElementById("preview-image");
  const loading = document.getElementById("loading");
  const errorBox = document.getElementById("error");
  const guestSelect = document.getElementById("preview-guest");
  const guestCount = document.getElementById("guest-count");
  const defaults = JSON.parse(document.getElementById("defaults").textContent);

  const GUEST_FIELDS = ["name", "postal_code", "address1", "address2", "joint_name", "honorific"];
  const STORAGE_KEY = "wedding-invitation-form";
  let previewDoc = "card";
  let previewUrl = null;
  let previewTimer = null;
  let previewSeq = 0;

  // ------------------------------------------------------------ 状態の保存
  function load() {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY)) || null;
    } catch {
      return null;
    }
  }
  function save(payload) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
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
    const current = guestSelect.value;
    guestSelect.innerHTML = "";
    list.forEach((g, i) => {
      const opt = document.createElement("option");
      opt.value = i;
      opt.textContent = `${i + 1}. ${g.name} ${g.honorific || "様"}`;
      guestSelect.appendChild(opt);
    });
    if (current && current < list.length) guestSelect.value = current;
    guestCount.textContent = `${list.length} 名`;
  }

  // ------------------------------------------------------------ 入力値
  function payload() {
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

  function fill(state) {
    const event = { ...defaults, ...(state?.event || {}) };
    for (const [key, value] of Object.entries(event)) {
      if (form.elements[key]) form.elements[key].value = value;
    }
    if (state?.design) {
      const radio = form.querySelector(`input[name=design][value="${state.design}"]`);
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

  // ------------------------------------------------------------ API
  async function post(url, body) {
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      let message = `エラーが発生しました (${res.status})`;
      try {
        message = (await res.json()).error || message;
      } catch { /* JSON 以外 */ }
      throw new Error(message);
    }
    return res.blob();
  }

  async function updatePreview() {
    const body = payload();
    if (!body.guests.length) {
      errorBox.textContent = "ゲストを 1 名以上入力してください";
      return;
    }
    const seq = ++previewSeq;
    loading.classList.add("show");
    try {
      const blob = await post(`/api/preview/${previewDoc}?guest=${guestSelect.value || 0}`, body);
      if (seq !== previewSeq) return;
      if (previewUrl) URL.revokeObjectURL(previewUrl);
      previewUrl = URL.createObjectURL(blob);
      image.src = previewUrl;
      errorBox.textContent = "";
    } catch (err) {
      if (seq === previewSeq) errorBox.textContent = err.message;
    } finally {
      if (seq === previewSeq) loading.classList.remove("show");
    }
  }

  function changed() {
    const body = payload();
    refreshGuestSelect(body.guests);
    save(body);
    clearTimeout(previewTimer);
    previewTimer = setTimeout(updatePreview, 600);
  }

  async function generate(doc, button) {
    const body = payload();
    if (!body.guests.length) {
      errorBox.textContent = "ゲストを 1 名以上入力してください";
      return;
    }
    const names = { zip: "wedding_invitation_print_data.zip", card: "invitation_cards.pdf", envelope: "envelopes.pdf" };
    button.disabled = true;
    errorBox.textContent = "";
    try {
      const blob = await post(`/api/generate/${doc}`, body);
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = names[doc];
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 10000);
    } catch (err) {
      errorBox.textContent = err.message;
    } finally {
      button.disabled = false;
    }
  }

  async function importCsv(file) {
    const fd = new FormData();
    fd.append("file", file);
    errorBox.textContent = "";
    try {
      const res = await fetch("/api/guests/csv", { method: "POST", body: fd });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "CSV を読み込めませんでした");
      if (!data.guests.length) throw new Error("CSV にゲストが見つかりませんでした");
      const replace = guests().length === 0 ||
        confirm(`${data.guests.length} 名を読み込みました。現在のゲスト一覧を置き換えますか？\n(キャンセルで末尾に追加)`);
      if (replace) rows.innerHTML = "";
      data.guests.forEach((g) => addGuest(g));
      changed();
    } catch (err) {
      errorBox.textContent = err.message;
    }
  }

  // ------------------------------------------------------------ イベント
  form.addEventListener("input", changed);
  form.addEventListener("change", changed);
  document.getElementById("add-guest").addEventListener("click", () => {
    if (rows.children.length >= window.MAX_GUESTS) return;
    addGuest().querySelector("input").focus();
  });
  document.getElementById("csv").addEventListener("change", (e) => {
    if (e.target.files[0]) importCsv(e.target.files[0]);
    e.target.value = "";
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

  fill(load());
  refreshGuestSelect(guests());
  updatePreview();
})();
