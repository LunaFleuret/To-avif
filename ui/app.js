// AVIF Batch Converter - Frontend Application Logic (Tauri v2)

const invoke = window.__TAURI__?.core?.invoke || (async (cmd, args) => {
  console.warn("Tauri invoke mockup:", cmd, args);
  return null;
});

const listen = window.__TAURI__?.event?.listen || ((evt, cb) => {
  console.warn("Tauri listen mockup:", evt);
  return () => {};
});

// Preset Definitions
const PRESETS = {
  anime: {
    crf: 20,
    cpu: 6,
    desc: "10-bit YUV 4:4:4 - Prevents color bleeding on line art and vibrant colors."
  },
  photo: {
    crf: 20,
    cpu: 8,
    desc: "10-bit YUV 4:2:0 - High compression ratio suitable for photographic images (Fast)."
  },
  lossless: {
    crf: 0,
    cpu: 6,
    desc: "Lossless encoding (CRF 0) - Retains original pixel perfection."
  },
  custom: {
    crf: 20,
    cpu: 6,
    desc: "User defined quality and parameters."
  }
};

// Application State
let state = {
  selectedPreset: "anime",
  cpuCount: 16,
  targetFiles: [],
  successfulOrigFiles: [],
  isConverting: false,
  menuRegistered: false,
  totalOrigBytes: 0,
  totalNewBytes: 0,
};

// DOM References
const el = {
  inputPath: document.getElementById("inputPath"),
  btnBrowseFolder: document.getElementById("btnBrowseFolder"),
  btnBrowseFiles: document.getElementById("btnBrowseFiles"),
  btnClearInput: document.getElementById("btnClearInput"),
  fileCountLabel: document.getElementById("fileCountLabel"),
  
  presetRadios: document.querySelectorAll('input[name="preset"]'),
  presetDescLabel: document.getElementById("presetDescLabel"),

  crfSlider: document.getElementById("crfSlider"),
  crfSpin: document.getElementById("crfSpin"),
  cpuSlider: document.getElementById("cpuSlider"),
  cpuSpin: document.getElementById("cpuSpin"),
  threadLabel: document.getElementById("threadLabel"),
  threadSlider: document.getElementById("threadSlider"),
  threadSpin: document.getElementById("threadSpin"),

  chkRecursive: document.getElementById("chkRecursive"),
  chkDeleteOrig: document.getElementById("chkDeleteOrig"),
  chkCustomOut: document.getElementById("chkCustomOut"),
  outPathEdit: document.getElementById("outPathEdit"),
  btnBrowseOut: document.getElementById("btnBrowseOut"),

  progressBar: document.getElementById("progressBar"),
  progressText: document.getElementById("progressText"),
  statLabel: document.getElementById("statLabel"),
  terminalContainer: document.getElementById("terminalContainer"),
  logText: document.getElementById("logText"),

  btnOpenContextMenu: document.getElementById("btnOpenContextMenu"),
  btnRecycleOrig: document.getElementById("btnRecycleOrig"),
  btnStart: document.getElementById("btnStart"),

  modalOverlay: document.getElementById("modalOverlay"),
  modalCloseBtn: document.getElementById("modalCloseBtn"),
  modalDismissBtn: document.getElementById("modalDismissBtn"),
  menuStatusText: document.getElementById("menuStatusText"),
  btnToggleMenuAction: document.getElementById("btnToggleMenuAction"),
};

// Log appending with color classes
function appendLog(text, className = "") {
  const span = document.createElement("span");
  if (className) span.className = className;
  span.textContent = text + "\n";
  el.logText.appendChild(span);
  el.terminalContainer.scrollTop = el.terminalContainer.scrollHeight;
}

function clearLog() {
  el.logText.innerHTML = "";
}

// 1. Preset & Sliders Interaction
function applyPreset(presetKey) {
  state.selectedPreset = presetKey;
  const p = PRESETS[presetKey];
  if (!p) return;

  el.presetDescLabel.textContent = p.desc;
  if (presetKey !== "custom") {
    el.crfSlider.value = p.crf;
    el.crfSpin.value = p.crf;
    el.cpuSlider.value = p.cpu;
    el.cpuSpin.value = p.cpu;
  }
}

el.presetRadios.forEach((radio) => {
  radio.addEventListener("change", (e) => {
    applyPreset(e.target.value);
  });
});

// Slider & Spin bidirectional bindings
function bindSliderSpin(slider, spin, onUserChange) {
  slider.addEventListener("input", (e) => {
    spin.value = e.target.value;
    if (onUserChange) onUserChange();
  });
  spin.addEventListener("input", (e) => {
    slider.value = e.target.value;
    if (onUserChange) onUserChange();
  });
}

function switchToCustom() {
  const customRadio = document.querySelector('input[name="preset"][value="custom"]');
  if (customRadio && !customRadio.checked) {
    customRadio.checked = true;
    state.selectedPreset = "custom";
    el.presetDescLabel.textContent = PRESETS.custom.desc;
  }
}

bindSliderSpin(el.crfSlider, el.crfSpin, switchToCustom);
bindSliderSpin(el.cpuSlider, el.cpuSpin, switchToCustom);
bindSliderSpin(el.threadSlider, el.threadSpin);

// 2. Custom Output toggle
el.chkCustomOut.addEventListener("change", (e) => {
  const on = e.target.checked;
  el.outPathEdit.disabled = !on;
  el.btnBrowseOut.disabled = !on;
  if (!on) {
    el.outPathEdit.value = "";
  }
});

// 3. Scan & File Detection (Async with progress & debounce)
let scanDebounceTimer = null;
let isScanning = false;

// Scan progress event listener from backend
listen("scan_progress", (event) => {
  if (isScanning && event.payload) {
    const count = event.payload.count || 0;
    el.fileCountLabel.textContent = `スキャン中: ${count.toLocaleString()} 件検出...`;
    el.fileCountLabel.style.color = "#f59e0b";
  }
});

function refreshFiles(immediate = false) {
  if (scanDebounceTimer) {
    clearTimeout(scanDebounceTimer);
    scanDebounceTimer = null;
  }

  const runScan = async () => {
    const raw = el.inputPath.value.trim();
    if (!raw) {
      state.targetFiles = [];
      el.fileCountLabel.textContent = "対象画像: 0 件";
      el.fileCountLabel.style.color = "#38bdf8";
      el.btnStart.disabled = true;
      isScanning = false;
      return;
    }

    const paths = raw.split(";").map((p) => p.trim()).filter(Boolean);
    const recursive = el.chkRecursive.checked;

    isScanning = true;
    el.fileCountLabel.textContent = "スキャン中...";
    el.fileCountLabel.style.color = "#f59e0b";
    el.btnStart.disabled = true;

    try {
      const res = await invoke("scan_paths", { paths, recursive });
      // 他の新しいスキャンによりキャンセルされた場合は無視
      if (!res) return;

      state.targetFiles = res.files;
      el.fileCountLabel.textContent = `対象画像: ${res.total_files.toLocaleString()} 件`;
      el.fileCountLabel.style.color = res.total_files > 0 ? "#38bdf8" : "#ef4444";
      el.btnStart.disabled = res.total_files === 0;
    } catch (err) {
      console.error("Scan error:", err);
      el.fileCountLabel.textContent = "スキャン失敗";
      el.fileCountLabel.style.color = "#ef4444";
    } finally {
      isScanning = false;
    }
  };

  if (immediate) {
    runScan();
  } else {
    scanDebounceTimer = setTimeout(runScan, 150);
  }
}

el.inputPath.addEventListener("input", () => refreshFiles(false));
el.chkRecursive.addEventListener("change", () => refreshFiles(true));

// Clear button
el.btnClearInput.addEventListener("click", () => {
  el.inputPath.value = "";
  state.targetFiles = [];
  el.fileCountLabel.textContent = "対象画像: 0 件";
  el.progressBar.style.width = "0%";
  el.progressText.textContent = "0%";
  el.statLabel.textContent = "準備完了";
  el.btnRecycleOrig.disabled = true;
  el.btnStart.disabled = true;
  clearLog();
});

// Browse folder
el.btnBrowseFolder.addEventListener("click", async () => {
  const dialog = window.__TAURI__?.dialog;
  if (dialog?.open) {
    const selected = await dialog.open({ directory: true, multiple: false });
    if (selected) {
      el.inputPath.value = selected;
      refreshFiles();
    }
  }
});

// Browse files
el.btnBrowseFiles.addEventListener("click", async () => {
  const dialog = window.__TAURI__?.dialog;
  if (dialog?.open) {
    const selected = await dialog.open({
      directory: false,
      multiple: true,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "heic", "heif", "jxl"] }],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      const cur = el.inputPath.value.trim();
      el.inputPath.value = cur ? `${cur}; ${paths.join("; ")}` : paths.join("; ");
      refreshFiles();
    }
  }
});

// Browse output folder
el.btnBrowseOut.addEventListener("click", async () => {
  const dialog = window.__TAURI__?.dialog;
  if (dialog?.open) {
    const selected = await dialog.open({ directory: true, multiple: false });
    if (selected) {
      el.outPathEdit.value = selected;
    }
  }
});

// 4. Drag & Drop handling
window.addEventListener("dragover", (e) => e.preventDefault());
window.addEventListener("drop", (e) => {
  e.preventDefault();
  if (e.dataTransfer && e.dataTransfer.files.length > 0) {
    const paths = [];
    for (let i = 0; i < e.dataTransfer.files.length; i++) {
      const f = e.dataTransfer.files[i];
      if (f.path) paths.push(f.path);
    }
    if (paths.length > 0) {
      el.inputPath.value = paths.join("; ");
      refreshFiles();
    }
  }
});

listen("tauri://drag-drop", (event) => {
  if (event.payload && event.payload.paths && event.payload.paths.length > 0) {
    el.inputPath.value = event.payload.paths.join("; ");
    refreshFiles();
  }
});

// 5. Conversion Progress Event Listener
listen("conversion-progress", (event) => {
  const p = event.payload;
  if (!p) return;

  const pct = Math.floor(p.percent);
  el.progressBar.style.width = `${pct}%`;
  el.progressText.textContent = `${pct}%`;
  el.statLabel.textContent = `処理中: ${p.current_index} / ${p.total_files} 件完了 (${pct}%)`;

  const res = p.last_result;
  if (res) {
    // Command line trace (Terminal style)
    if (res.command) {
      appendLog(`[CMD] ${res.command}`, "log-line-cmd");
    }

    if (res.success) {
      state.successfulOrigFiles.push(res.src_path);
      state.totalOrigBytes += res.original_bytes;
      state.totalNewBytes += res.converted_bytes;

      const origKb = (res.original_bytes / 1024).toFixed(1);
      const newKb = (res.converted_bytes / 1024).toFixed(1);
      const red = res.original_bytes > 0 ? ((1 - res.converted_bytes / res.original_bytes) * 100).toFixed(1) : 0;
      
      const tags = [];
      if (res.metadata_copied) tags.push("Exif");
      if (res.timestamps_copied) tags.push("日時");
      const tagStr = tags.length > 0 ? ` [${tags.join("+")}保持]` : "";

      appendLog(`[${p.current_index}/${p.total_files}] 成功: ${res.filename} (${origKb}KB -> ${newKb}KB, -${red}%)${tagStr}`, "log-line-success");
    } else {
      appendLog(`[${p.current_index}/${p.total_files}] 失敗: ${res.filename} (${res.error_message || "エラー"})`, "log-line-error");
    }
  }

  if (p.batch_message) {
    appendLog(p.batch_message, "log-line-info");
  }
});

// 6. Start & Cancel Conversion
el.btnStart.addEventListener("click", async () => {
  if (state.isConverting) {
    // Cancel request
    await invoke("cancel_conversion");
    el.statLabel.textContent = "中断リクエスト送信中...";
    return;
  }

  if (state.targetFiles.length === 0) return;

  state.isConverting = true;
  state.successfulOrigFiles = [];
  state.totalOrigBytes = 0;
  state.totalNewBytes = 0;

  clearLog();
  el.progressBar.style.width = "0%";
  el.progressText.textContent = "0%";
  el.btnRecycleOrig.disabled = true;

  el.btnStart.textContent = "中断";
  el.btnStart.className = "btn btn-danger";
  el.statLabel.textContent = "変換処理を実行中...";

  const total = state.targetFiles.length;
  const workers = parseInt(el.threadSpin.value, 10) || 8;
  appendLog(`[準備] 変換タスクを開始しました (対象: ${total}件, スレッド数: ${workers})`, "log-line-info");

  const files = state.targetFiles.map((f) => f.path);
  const outDir = el.chkCustomOut.checked && el.outPathEdit.value.trim() ? el.outPathEdit.value.trim() : null;

  const options = {
    preset_id: state.selectedPreset,
    crf: parseInt(el.crfSpin.value, 10) || 20,
    cpu_used: parseInt(el.cpuSpin.value, 10) || 6,
    thread_count: workers,
    preserve_metadata: true,
    preserve_timestamps: true,
    trash_originals: el.chkDeleteOrig.checked,
    output_dir: outDir,
  };

  try {
    const results = await invoke("run_batch_conversion", { files, options });
    const successCount = results.filter((r) => r.success).length;
    const failCount = results.length - successCount;

    const origMb = (state.totalOrigBytes / (1024 * 1024)).toFixed(2);
    const newMb = (state.totalNewBytes / (1024 * 1024)).toFixed(2);
    const savedMb = ((state.totalOrigBytes - state.totalNewBytes) / (1024 * 1024)).toFixed(2);
    const ratio = state.totalOrigBytes > 0 ? (((state.totalOrigBytes - state.totalNewBytes) / state.totalOrigBytes) * 100).toFixed(1) : 0;

    const summaryStr = `完了: 成功 ${successCount}件 / 失敗 ${failCount}件 | 元: ${origMb}MB -> AVIF: ${newMb}MB (削減: ${savedMb}MB / -${ratio}%)`;
    el.statLabel.textContent = summaryStr;
    appendLog(`\n=== 変換完了 ===\n${summaryStr}`, "log-line-summary");

    // Auto trash if enabled
    if (el.chkDeleteOrig.checked && state.successfulOrigFiles.length > 0) {
      appendLog(`[ごみ箱] 成功した元ファイル (${state.successfulOrigFiles.length}件) を自動でごみ箱へ移動中...`, "log-line-info");
      const moved = await invoke("trash_files", { paths: state.successfulOrigFiles });
      appendLog(`[ごみ箱] ${moved}件の元ファイルをごみ箱へ移動しました。`, "log-line-info");
      el.btnRecycleOrig.disabled = true;
    } else if (state.successfulOrigFiles.length > 0) {
      el.btnRecycleOrig.disabled = false;
    }
  } catch (err) {
    appendLog(`[エラー] ${err}`, "log-line-error");
    el.statLabel.textContent = `エラー: ${err}`;
  } finally {
    state.isConverting = false;
    el.btnStart.textContent = "変換開始";
    el.btnStart.className = "btn btn-primary";
  }
});

// 7. Manual Recycle Bin move
el.btnRecycleOrig.addEventListener("click", async () => {
  if (state.successfulOrigFiles.length === 0) return;
  const count = state.successfulOrigFiles.length;
  if (!confirm(`変換に成功した元ファイル (${count}件) をゴミ箱へ移動しますか？\n（ゴミ箱から元に戻せます）`)) {
    return;
  }

  try {
    const moved = await invoke("trash_files", { paths: state.successfulOrigFiles });
    appendLog(`[ごみ箱] ${moved}件の元ファイルをゴミ箱へ移動しました。`, "log-line-info");
    el.btnRecycleOrig.disabled = true;
    alert(`${moved} 件の元ファイルをゴミ箱へ移動しました。`);
  } catch (err) {
    alert(`ゴミ箱移動エラー: ${err}`);
  }
});

// 8. Context Menu Settings Modal
async function updateMenuStatus() {
  try {
    const reg = await invoke("get_menu_status");
    state.menuRegistered = reg;
    el.menuStatusText.textContent = reg ? "登録済み" : "未登録";
    el.menuStatusText.style.color = reg ? "#10b981" : "#a1a1aa";
    el.btnToggleMenuAction.textContent = reg ? "登録を解除する" : "登録する";
    el.btnToggleMenuAction.className = reg ? "btn btn-danger" : "btn btn-primary";
  } catch (e) {
    console.error("Menu status error:", e);
  }
}

el.btnOpenContextMenu.addEventListener("click", () => {
  el.modalOverlay.classList.remove("hidden");
  updateMenuStatus();
});

el.modalCloseBtn.addEventListener("click", () => el.modalOverlay.classList.add("hidden"));
el.modalDismissBtn.addEventListener("click", () => el.modalOverlay.classList.add("hidden"));

el.btnToggleMenuAction.addEventListener("click", async () => {
  const target = !state.menuRegistered;
  try {
    const res = await invoke("set_menu_registered", { register: target });
    state.menuRegistered = res;
    await updateMenuStatus();
  } catch (err) {
    alert(`レジストリ操作エラー: ${err}`);
  }
});

// 9. App Initialization
async function init() {
  try {
    const count = await invoke("get_cpu_count");
    if (count) {
      state.cpuCount = count;
      el.threadLabel.textContent = `並列処理スレッド数 (CPU検出: ${count}スレッド):`;
      el.threadSlider.max = Math.max(16, count);
      el.threadSpin.max = Math.max(16, count);
      const defWorkers = Math.min(count, 8);
      el.threadSlider.value = defWorkers;
      el.threadSpin.value = defWorkers;
    }
  } catch (e) {
    console.error("CPU count error:", e);
  }

  // Check initial CLI argument paths
  try {
    const initPaths = await invoke("get_initial_paths");
    if (initPaths && initPaths.length > 0) {
      el.inputPath.value = initPaths.join("; ");
      refreshFiles();
    }
  } catch (e) {
    console.error("Initial paths error:", e);
  }
}

document.addEventListener("DOMContentLoaded", init);
