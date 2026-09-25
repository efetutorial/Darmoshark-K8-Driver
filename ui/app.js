const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

const EFFECTS = {
  off: "Off", solid: "Solid", breathing: "Breathing", neon: "Neon",
  wave: "Wave", ripple: "Ripple", raindrop: "Raindrop", snake: "Snake",
  reactive: "Reactive", convergence: "Convergence", sine: "Sine wave", kaleidoscope: "Kaleidoscope",
  line_wave: "Line wave", user_picture: "Custom keys", laser: "Laser",
  circle_wave: "Circle wave", dazzling: "Dazzling", rain_down: "Rain",
  meteor: "Meteor", reactive_off: "Reactive fade", music_3: "Audio spectrum",
  screen_color: "Screen color", music_2: "Audio bars"
};

const KEYBOARD_LAYOUT = [
  [{n:"Esc"},{gap:.55},{n:"F1"},{n:"F2"},{n:"F3"},{n:"F4"},{gap:.25},{n:"F5"},{n:"F6"},{n:"F7"},{n:"F8"},{gap:.25},{n:"F9"},{n:"F10"},{n:"F11"},{n:"F12"},{n:"PrintScreen",l:"Prt"},{n:"Delete",l:"Del"}],
  [{n:"Grave",l:"`"},{n:"1"},{n:"2"},{n:"3"},{n:"4"},{n:"5"},{n:"6"},{n:"7"},{n:"8"},{n:"9"},{n:"0"},{n:"Minus",l:"−"},{n:"Equal",l:"="},{n:"Backspace",l:"Back",w:2},{n:"Home",l:"Home"}],
  [{n:"Tab",w:1.5},{n:"Q"},{n:"W"},{n:"E"},{n:"R"},{n:"T"},{n:"Y"},{n:"U"},{n:"I"},{n:"O"},{n:"P"},{n:"LeftBracket",l:"["},{n:"RightBracket",l:"]"},{n:"Backslash",l:"\\",w:1.5},{n:"PageUp",l:"Pg↑"}],
  [{n:"CapsLock",l:"Caps",w:1.8},{n:"A"},{n:"S"},{n:"D"},{n:"F"},{n:"G"},{n:"H"},{n:"J"},{n:"K"},{n:"L"},{n:"Semicolon",l:";"},{n:"Quote",l:"'"},{n:"Enter",l:"Enter",w:2.2},{n:"PageDown",l:"Pg↓"}],
  [{n:"LeftShift",l:"Shift",w:1.4},{n:"NonUSBackslash",l:"<"},{n:"Z"},{n:"X"},{n:"C"},{n:"V"},{n:"B"},{n:"N"},{n:"M"},{n:"Comma",l:","},{n:"Period",l:"."},{n:"Slash",l:"/"},{n:"RightShift",l:"Shift",w:1.5},{n:"Up",l:"↑"},{n:"End",l:"End"}],
  [{n:"LeftCtrl",l:"Ctrl",w:1.3},{n:"LeftMeta",l:"Meta",w:1.2},{n:"LeftAlt",l:"Alt",w:1.2},{n:"Space",l:"",w:5.8},{n:"RightAlt",l:"Alt",w:1.2},{n:null,l:"Fn",w:1.1},{n:"RightCtrl",l:"Ctrl",w:1.1},{n:"Left",l:"←"},{n:"Down",l:"↓"},{n:"Right",l:"→"}]
];

const state = {
  base: "", device: null, status: null, keys: {}, targets: [], profile: 0,
  selectedKey: null, mapLayer: "normal", displayFile: null, macroEvents: [],
  recording: false, recordStart: 0, lastRecordAt: 0,
  live: {stream:null, timer:null, audio:null, inFlight:false, native:false}, activities: []
};

function nowTime() { return new Intl.DateTimeFormat("en", {hour:"2-digit", minute:"2-digit", second:"2-digit"}).format(new Date()); }

function addActivity(title, detail = "", error = false) {
  state.activities.unshift({title, detail, error, time: nowTime()});
  state.activities = state.activities.slice(0, 80);
  renderActivities();
}

function toast(title, detail = "", error = false) {
  addActivity(title, detail, error);
  const item = document.createElement("div");
  item.className = `toast${error ? " error" : ""}`;
  item.innerHTML = `<i></i><div><b></b><span></span></div>`;
  $("b", item).textContent = title;
  $("span", item).textContent = detail;
  $("#toast-stack").replaceChildren(item);
  setTimeout(() => item.remove(), 4300);
}

function renderActivities() {
  const root = $("#activity-list");
  root.innerHTML = "";
  if (!state.activities.length) {
    root.innerHTML = '<div class="empty-events">No activity yet.</div>';
    return;
  }
  state.activities.forEach(entry => {
    const item = document.createElement("div");
    item.className = `activity-item${entry.error ? " error" : ""}`;
    item.innerHTML = `<i></i><div><b></b><span></span></div>`;
    $("b", item).textContent = entry.title;
    $("span", item).textContent = `${entry.time}${entry.detail ? ` · ${entry.detail}` : ""}`;
    root.append(item);
  });
}

async function resolveBackend() {
  if (window.__TAURI__?.core?.invoke) {
    state.base = "tauri";
  } else {
    state.base = location.protocol.startsWith("http") ? location.origin : "http://127.0.0.1:47691";
  }
}

async function api(path, options = {}) {
  if (window.__TAURI__?.core?.invoke) {
    const url = new URL(path, "http://localhost");
    const query = Object.fromEntries(url.searchParams);
    let bodyValue = null;
    let bytes = null;
    if (options.body !== undefined) {
      if (typeof options.body === "string") {
        bodyValue = JSON.parse(options.body);
      } else if (options.body instanceof Blob) {
        bytes = [...new Uint8Array(await options.body.arrayBuffer())];
        if (options.body.name) query.filename = options.body.name;
      } else if (ArrayBuffer.isView(options.body)) {
        bytes = [...new Uint8Array(options.body.buffer, options.body.byteOffset, options.body.byteLength)];
      } else if (options.body instanceof ArrayBuffer) {
        bytes = [...new Uint8Array(options.body)];
      } else {
        bodyValue = options.body;
      }
    }
    try {
      return await window.__TAURI__.core.invoke("api", {path:url.pathname, bodyValue, bytes, query});
    } catch (error) {
      throw new Error(typeof error === "string" ? error : error?.message || String(error));
    }
  }
  const response = await fetch(state.base + path, options);
  let data = {};
  try { data = await response.json(); } catch { /* Response need not be JSON. */ }
  if (!response.ok || data.ok === false) throw new Error(data.error || `HTTP ${response.status}`);
  return data;
}

function post(path, body = {}) {
  return api(path, {method:"POST", headers:{"Content-Type":"application/json"}, body:JSON.stringify(body)});
}

async function withBusy(button, task) {
  const old = button.textContent;
  button.disabled = true;
  button.textContent = "Applying…";
  try { return await task(); }
  finally { button.disabled = false; button.textContent = old; }
}

function navigate(page) {
  $$(".page").forEach(item => item.classList.toggle("active", item.id === page));
  $$(".nav-item").forEach(item => item.classList.toggle("active", item.dataset.page === page));
  const button = $(`.nav-item[data-page="${page}"]`);
  const label = button?.lastElementChild?.textContent || page;
  $("#page-title").textContent = label;
  if (location.hash.slice(1) !== page) {
    history.replaceState(null, "", `#${page}`);
  }
  window.scrollTo({top: 0, behavior: "smooth"});
}

function renderKeyboard(root, interactive = false) {
  root.innerHTML = "";
  KEYBOARD_LAYOUT.forEach(row => {
    const line = document.createElement("div");
    line.className = "keyboard-row";
    row.forEach(key => {
      if (key.gap) {
        const gap = document.createElement("span");
        gap.style.flex = key.gap;
        line.append(gap);
        return;
      }
      const button = document.createElement("button");
      button.type = "button";
      button.className = "keycap";
      button.style.setProperty("--w", key.w || 1);
      button.textContent = key.l ?? key.n;
      if (key.n) button.dataset.key = key.n;
      else button.disabled = true;
      if (interactive && key.n) button.addEventListener("click", () => selectKey(key.n));
      line.append(button);
    });
    root.append(line);
  });
}

function selectKey(key) {
  state.selectedKey = key;
  $$("#key-map .keycap").forEach(item => item.classList.toggle("selected", item.dataset.key === key));
  $("#selected-key-title").textContent = `${key}`;
  $("#selected-key-chip").textContent = key;
  $("#selected-layer-chip").textContent = `${state.mapLayer === "fn" ? "FN" : "NORMAL"} LAYER`;
  $("#paint-key").textContent = displayKeyName(key);
}

function displayKeyName(key) {
  const found = KEYBOARD_LAYOUT.flat().find(item => item.n === key);
  return found?.l || key || "—";
}

function populateDeviceData(data) {
  state.device = data;
  state.keys = data.keys || {};
  state.targets = data.targets || [];
  const keyList = $("#key-list"), targetList = $("#target-list");
  keyList.innerHTML = ""; targetList.innerHTML = "";
  Object.entries(state.keys).forEach(([name, usage]) => {
    const option = document.createElement("option");
    option.value = name;
    keyList.append(option); targetList.append(option.cloneNode(true));
  });
  ["disabled", ...state.targets].forEach(name => {
    const option = document.createElement("option"); option.value = name; targetList.append(option);
  });
  const connected = Boolean(data.connected);
  $("#connection-dot").classList.toggle("online", connected);
  $("#offline-banner").classList.toggle("hidden", connected);
  $("#device-name").textContent = connected ? "Darmoshark K8" : "Disconnected";
  $("#device-path").textContent = connected ? "Connected" : "Check USB connection";
}

async function refreshDevice(silent = false) {
  try {
    const device = await api("/api/device");
    populateDeviceData(device);
    if (device.connected && (!silent || !state.status)) await refreshStatus(true);
    if (!device.connected) state.status = null;
    if (!silent) toast("Keyboard refreshed", device.connected ? "Connected" : "Keyboard not found", !device.connected);
  } catch (error) {
    $("#connection-dot").classList.remove("online");
    $("#offline-banner").classList.remove("hidden");
    if (!silent) toast("Cannot connect to keyboard", error.message, true);
  }
}

async function refreshStatus(silent = false) {
  try {
    const data = await api("/api/status");
    state.status = data.status;
    state.profile = data.status.profile;
    applyStatus(data.status);
    if (!silent) toast("Keyboard refreshed", `Firmware ${data.status.firmware}`);
  } catch (error) {
    if (!silent) toast("Cannot read keyboard settings", error.message, true);
    throw error;
  }
}

function applyStatus(status) {
  $$(".profile-switch button").forEach(button => button.classList.toggle("active", +button.dataset.profile === status.profile));

  if ([...$("#effect").options].some(option => option.value === status.lighting.effect)) $("#effect").value = status.lighting.effect;
  setColor(status.lighting.color);
  $("#brightness").value = status.lighting.brightness;
  $("#speed").value = status.lighting.speed;
  updateEffectFields();
  $("#direction").value = status.lighting.direction;
  $("#rgb-slot").value = status.lighting.slot || 1;
  $("#rainbow").checked = status.lighting.rainbow;
  updateRangeLabels(); updateEffectFields();

  $("#report-rate").value = status.report_rate;
  $("#debounce").value = status.debounce;
  $("#debounce-value").textContent = `${status.debounce} ms`;
  Object.entries(status.options).forEach(([key, value]) => {
    const input = $(`[data-option="${key}"]`); if (input) input.checked = value;
  });
  $("#sleep-bt").value = Math.round(status.sleep.bluetooth / 60);
  $("#sleep-wireless").value = Math.round(status.sleep.wireless / 60);
  $("#sleep-bt-deep").value = Math.round(status.sleep.bluetooth_deep / 60);
  $("#sleep-wireless-deep").value = Math.round(status.sleep.wireless_deep / 60);
}

function setColor(value) {
  const color = /^#[0-9a-f]{6}$/i.test(value) ? value : "#ff6432";
  $("#color").value = color; $("#color-hex").value = color.toUpperCase();
  document.documentElement.style.setProperty("--rgb", color);
}

function setKeyColor(value) {
  const color = /^#[0-9a-f]{6}$/i.test(value) ? value : "#ff6432";
  $("#key-color").value = color; $("#key-color-hex").value = color.toUpperCase();
  document.documentElement.style.setProperty("--paint", color);
}

function updateRangeLabels() {
  $("#brightness-value").textContent = `${$("#brightness").value} / 4`;
  $("#speed-value").textContent = `${$("#speed").value} / 4`;
}

// Capabilities match the official K8 yc200CommonLayoutFour definition.
// Direction values use the protocol's shared nibble names, with effect-specific labels.
const EFFECT_CAPABILITIES = {
  off: {},
  solid: {color:true, rainbow:true, brightness:true},
  neon: {brightness:true, speed:true},
  user_picture: {brightness:true, slot:true},
  screen_color: {note:"Start Screen color below to sync your display."},
  wave: {directions:["Right", "Left", "Down", "Up"]},
  snake: {directions:["Zigzag", "Return"]},
  kaleidoscope: {directions:["Outward", "Inward"]},
  line_wave: {directions:["Right", "Left"]},
  circle_wave: {directions:["Counterclockwise", "Clockwise"]},
  music_2: {color:true, rainbow:true, brightness:true, directions:["Upright", "Separate", "Intersect"], note:"Start Audio spectrum below to sync your audio."},
  music_3: {color:true, rainbow:true, brightness:true, directions:["Upright", "Separate", "Intersect"], note:"Start Audio spectrum below to sync your audio."}
};

function effectCapabilities() {
  const effect = $("#effect").value;
  const animated = {color:true, rainbow:true, brightness:true, speed:true};
  const caps = EFFECT_CAPABILITIES[effect];
  if (["wave", "snake", "kaleidoscope", "line_wave", "circle_wave"].includes(effect)) return {...animated, ...caps};
  return caps || animated;
}

function updateEffectFields() {
  const caps = effectCapabilities();
  const visible = {color:caps.color, rainbow:caps.rainbow,
    brightness:caps.brightness, speed:caps.speed, direction:!!caps.directions, "rgb-slot":caps.slot};
  Object.entries(visible).forEach(([name, show]) => {
    const wrapper = $(`#${name}-wrap`);
    wrapper.classList.toggle("hidden", !show);
    $$("input, select", wrapper).forEach(input => input.disabled = !show);
  });
  const direction = $("#direction"), previous = direction.value;
  direction.replaceChildren();
  (caps.directions || []).forEach((label, index) => {
    const option = document.createElement("option");
    option.value = ["right", "left", "down", "up"][index];
    option.textContent = label;
    direction.append(option);
  });
  if ([...direction.options].some(option => option.value === previous)) direction.value = previous;
  $("#effect-note").textContent = caps.note || "";
  $("#effect-note").classList.toggle("hidden", !caps.note);
  $$(".controls-panel .field-row").forEach(row => row.classList.toggle("hidden", [...row.children].every(child => child.classList.contains("hidden"))));
  const effect = $("#effect").value;
  $("#rgb-keyboard").classList.toggle("rainbow-preview", effect === "neon" || !!(caps.rainbow && $("#rainbow").checked));
  $("#rgb-keyboard").classList.toggle("unlit-preview", ["off", "screen_color", "user_picture"].includes(effect));
  $("#color-wrap").classList.toggle("rainbow-selected", !!(caps.rainbow && $("#rainbow").checked));
}

function lightingPayload() {
  const caps = effectCapabilities();
  return {effect:$("#effect").value, color:caps.color && !$("#rainbow").checked ? $("#color").value : "#ffffff",
    brightness:caps.brightness ? +$("#brightness").value : 4,
    speed:caps.speed ? +$("#speed").value : 2,
    direction:caps.directions ? $("#direction").value : "right",
    rainbow:!!(caps.rainbow && $("#rainbow").checked),
    slot:caps.slot ? +$("#rgb-slot").value : 1};
}

async function applyLighting(button = $("#apply-lighting"), quiet = false) {
  const payload = lightingPayload();
  await stopLive();
  await withBusy(button, () => post("/api/rgb", payload));
  if (!quiet) toast("Lighting applied", EFFECTS[payload.effect] || payload.effect);
}

function renderPresets() {
  const presets = JSON.parse(localStorage.getItem("k8-presets") || "[]");
  const root = $("#preset-list"); root.innerHTML = "";
  if (!presets.length) { root.innerHTML = '<span class="empty-presets">No presets yet</span>'; return; }
  presets.forEach((preset, index) => {
    const item = document.createElement("button"); item.className = "preset-chip";
    item.innerHTML = `<i></i><span><b></b><small></small></span><button class="remove-preset" title="Delete">×</button>`;
    item.style.setProperty("--preset", preset.color); $("b", item).textContent = preset.name;
    $("small", item).textContent = EFFECTS[preset.effect] || preset.effect;
    item.addEventListener("click", async event => {
      if (event.target.closest(".remove-preset")) return;
      $("#effect").value = preset.effect; setColor(preset.color); $("#brightness").value = preset.brightness;
      $("#rgb-slot").value = preset.slot || 1;
      updateEffectFields();
      $("#speed").value = preset.speed; $("#direction").value = preset.direction; $("#rainbow").checked = preset.rainbow;
      updateRangeLabels(); updateEffectFields();
      try { await applyLighting(undefined, true); toast("Preset applied", preset.name); } catch (error) { toast("Could not apply preset", error.message, true); }
    });
    $(".remove-preset", item).addEventListener("click", event => { event.stopPropagation(); presets.splice(index,1); localStorage.setItem("k8-presets",JSON.stringify(presets)); renderPresets(); });
    root.append(item);
  });
}

async function stopLive(notifyBackend = true) {
  const native = state.live.native;
  if (state.live.timer) clearInterval(state.live.timer);
  if (state.live.stream) state.live.stream.getTracks().forEach(track => track.stop());
  if (state.live.audio) state.live.audio.close();
  state.live = {stream:null,timer:null,audio:null,inFlight:false,native:false};
  $("#lighting-live-label").textContent = "PREVIEW";
  $("#live-stop").disabled = true;
  if (native && notifyBackend) {
    try { await post("/api/live-native-stop"); } catch { /* Session may already be gone. */ }
  }
}

async function pushLive(path, data) {
  if (state.live.inFlight) return;
  state.live.inFlight = true;
  try { await post(path, data); }
  catch (error) { await stopLive(); toast("Live sync stopped", error.message, true); }
  finally { state.live.inFlight = false; }
}

function chooseDisplayFile(file) {
  if (!file) return;
  if (file.size > 100 * 1024 * 1024) return toast("Image exceeds 100 MB", "", true);
  if (window.k8Display) window.k8Display.selectImage();
  state.displayFile = file;
  const preview = $("#display-preview"); if (preview.src.startsWith("blob:")) URL.revokeObjectURL(preview.src); preview.src = URL.createObjectURL(file); preview.classList.add("visible");
  $("#display-placeholder").classList.add("hidden");
  $("#file-meta").textContent = file.name;
  $("#frame-meta").textContent = `${(file.size / 1024).toFixed(file.size > 1024 * 1024 ? 0 : 1)} KB`;
  toast("Image selected", file.name);
}

async function pollProgress() {
  try {
    const progress = await api("/api/progress");
    $("#upload-progress-bar").style.width = `${progress.value * 100}%`;
    $("#upload-progress-label").textContent = progress.label || "Ready";
  } catch { /* Upload itself reports errors. */ }
}

function usageFor(value) {
  if (!value) return 0;
  if (state.keys[value] !== undefined) return state.keys[value];
  const numeric = Number(value);
  if (Number.isInteger(numeric) && numeric >= 0 && numeric <= 255) return numeric;
  throw new Error(`Unknown key: ${value}`);
}

function renderMacroEvents() {
  const root = $("#macro-events"); root.innerHTML = "";
  $("#macro-count").textContent = `${state.macroEvents.length} events`;
  if (!state.macroEvents.length) { root.innerHTML = '<div class="empty-events">Record a sequence or add an event.</div>'; return; }
  state.macroEvents.forEach((event, index) => {
    const row = document.createElement("div"); row.className = "macro-event";
    row.innerHTML = `<input class="event-key" list="key-list"><select class="event-action"><option value="down">Press</option><option value="up">Release</option></select><div class="unit-field"><input class="event-delay" type="number" min="0" max="65535"><em>ms</em></div><button title="Delete">×</button>`;
    $(".event-key", row).value = event.key; $(".event-action", row).value = event.action; $(".event-delay", row).value = event.delay;
    $(".event-key", row).addEventListener("change", e => event.key = e.target.value);
    $(".event-action", row).addEventListener("change", e => event.action = e.target.value);
    $(".event-delay", row).addEventListener("change", e => event.delay = Math.max(0, Math.min(65535, +e.target.value)));
    $("button", row).addEventListener("click", () => { state.macroEvents.splice(index,1); renderMacroEvents(); });
    root.append(row);
  });
}

function eventKeyName(event) {
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
  if (/^Digit[0-9]$/.test(event.code)) return event.code.slice(5);
  if (/^F(?:[1-9]|1[0-2])$/.test(event.code)) return event.code;
  return {Escape:"Esc",Enter:"Enter",Space:"Space",Tab:"Tab",Backspace:"Backspace",Minus:"Minus",Equal:"Equal",
    BracketLeft:"LeftBracket",BracketRight:"RightBracket",Backslash:"Backslash",Semicolon:"Semicolon",Quote:"Quote",
    Backquote:"Grave",Comma:"Comma",Period:"Period",Slash:"Slash",CapsLock:"CapsLock",PrintScreen:"PrintScreen",
    ScrollLock:"ScrollLock",Pause:"Pause",Insert:"Insert",Home:"Home",PageUp:"PageUp",Delete:"Delete",End:"End",
    PageDown:"PageDown",ArrowRight:"Right",ArrowLeft:"Left",ArrowDown:"Down",ArrowUp:"Up"}[event.code] || null;
}

function handleRecording(event, action) {
  if (!state.recording) return;
  event.preventDefault(); event.stopPropagation();
  if (event.code === "Escape" && action === "down") { stopRecording(); return; }
  if (event.repeat) return;
  const key = eventKeyName(event); if (!key) return;
  const time = performance.now();
  if (state.macroEvents.length) state.macroEvents[state.macroEvents.length - 1].delay = Math.min(65535, Math.max(0, Math.round(time - state.lastRecordAt)));
  state.macroEvents.push({key, action, delay:30}); state.lastRecordAt = time;
  renderMacroEvents();
}

function stopRecording() {
  state.recording = false; $("#recording-hint").classList.add("hidden");
  $("#macro-record").textContent = "● Record"; toast("Recording saved", `${state.macroEvents.length} events`);
}

function openDrawer(open) {
  $("#activity-drawer").classList.toggle("open", open); $("#activity-drawer").setAttribute("aria-hidden", String(!open));
  $("#drawer-scrim").classList.toggle("open", open);
}

const COMMANDS = [
  ["Lighting", "Edit lighting", "lighting"],
  ["Display", "Upload an image or GIF", "display"], ["Keys", "Assign keys and colors", "keys"],
  ["Macros", "Record a macro", "macros"], ["Settings", "Performance and power", "settings"]
];

function openCommands(open = true) {
  $("#command-modal").classList.toggle("hidden", !open);
  if (open) { $("#command-search").value = ""; renderCommands(); setTimeout(() => $("#command-search").focus(), 10); }
}

function renderCommands() {
  const query = $("#command-search").value.toLocaleLowerCase("en");
  const root = $("#command-results"); root.innerHTML = "";
  COMMANDS.filter(item => item.join(" ").toLocaleLowerCase("en").includes(query)).forEach((item,index) => {
    const button = document.createElement("button"); button.className = `command-result${index === 0 ? " active" : ""}`;
    button.innerHTML = `<b></b><span></span>`; $("b",button).textContent=item[0]; $("span",button).textContent=item[1];
    button.addEventListener("click",()=>{navigate(item[2]);openCommands(false)}); root.append(button);
  });
}

function wireEvents() {
  $$(".nav-item,.jump").forEach(button => button.addEventListener("click", () => navigate(button.dataset.page)));
  $("#refresh").addEventListener("click", () => refreshDevice());
  $$(".profile-switch button").forEach(button => button.addEventListener("click", async () => {
    try { await post("/api/profile", {profile:+button.dataset.profile}); state.profile=+button.dataset.profile; await refreshStatus(true); toast("Profile changed", `Profile ${state.profile+1}`); }
    catch(error){toast("Could not change profile",error.message,true)}
  }));

  const chooseColor = value => { $("#rainbow").checked=false; setColor(value); updateEffectFields(); };
  $("#color").addEventListener("input", e=>chooseColor(e.target.value));
  $("#color-hex").addEventListener("change", e=>chooseColor(e.target.value));
  $$("#color-palette button").forEach(button=>button.addEventListener("click",()=>chooseColor(button.dataset.color)));
  $("#brightness").addEventListener("input", updateRangeLabels); $("#speed").addEventListener("input", updateRangeLabels);
  $("#effect").addEventListener("change", updateEffectFields);
  $("#rainbow").addEventListener("change", updateEffectFields);
  $("#apply-lighting").addEventListener("click", async event=>{try{await applyLighting(event.currentTarget)}catch(error){toast("Could not apply lighting",error.message,true)}});
  $("#save-preset").addEventListener("click",()=>{const name=prompt("Preset name:",`Preset ${new Date().toLocaleTimeString("en",{hour:"2-digit",minute:"2-digit"})}`);if(!name)return;const presets=JSON.parse(localStorage.getItem("k8-presets")||"[]");presets.push({name,...lightingPayload()});localStorage.setItem("k8-presets",JSON.stringify(presets.slice(-20)));renderPresets();toast("Preset saved",name)});

  $("#live-stop").addEventListener("click",async()=>{await stopLive();toast("Live sync stopped")});
  $("#live-screen").addEventListener("click",async()=>{await stopLive();try{if(window.__TAURI__?.core?.invoke){await post("/api/live-native-start",{kind:"screen"});state.live.native=true;$("#live-stop").disabled=false;$("#effect").value="screen_color";updateEffectFields();$("#lighting-live-label").textContent="SCREEN SYNC";toast("Screen sync started");return}const stream=await navigator.mediaDevices.getDisplayMedia({video:true,audio:false});state.live.stream=stream;$("#live-stop").disabled=false;const video=$("#capture-video");video.srcObject=stream;await video.play();stream.getVideoTracks()[0].addEventListener("ended",()=>stopLive());$("#effect").value="screen_color";updateEffectFields();await post("/api/rgb",{...lightingPayload(),effect:"screen_color"});const canvas=$("#capture-canvas"),context=canvas.getContext("2d",{willReadFrequently:true});state.live.timer=setInterval(()=>{context.drawImage(video,0,0,1,1);pushLive("/api/live-screen",{rgba:[...context.getImageData(0,0,1,1).data]})},40);$("#lighting-live-label").textContent="SCREEN SYNC";toast("Screen sync started")}catch(error){await stopLive();toast("Could not start screen capture",error.message,true)}});
  $("#live-audio").addEventListener("click",async()=>{await stopLive();try{if(window.__TAURI__?.core?.invoke){await post("/api/live-native-start",{kind:"audio"});state.live.native=true;$("#live-stop").disabled=false;$("#effect").value="music_3";updateEffectFields();$("#lighting-live-label").textContent="AUDIO SYNC";toast("Audio sync started");return}let stream;try{stream=await navigator.mediaDevices.getDisplayMedia({video:true,audio:true})}catch{stream=await navigator.mediaDevices.getUserMedia({audio:true})}if(!stream.getAudioTracks().length){stream.getTracks().forEach(track=>track.stop());stream=await navigator.mediaDevices.getUserMedia({audio:true})}state.live.stream=stream;$("#live-stop").disabled=false;stream.getTracks().forEach(track=>track.addEventListener("ended",()=>stopLive()));const audio=new AudioContext();state.live.audio=audio;const analyser=audio.createAnalyser();analyser.fftSize=2048;analyser.smoothingTimeConstant=.4;audio.createMediaStreamSource(stream).connect(analyser);const spectrum=new Uint8Array(analyser.frequencyBinCount);const effect=$("#effect").value==="music_2"?"music_2":"music_3";$("#effect").value=effect;updateEffectFields();await post("/api/rgb",{...lightingPayload(),effect});state.live.timer=setInterval(()=>{analyser.getByteFrequencyData(spectrum);const first=[...spectrum.slice(0,100)],base=Math.min(...first.slice(20,40)),adjusted=first.map(v=>v-base),peak=Math.max(...adjusted,1),levels=adjusted.map(v=>Math.max(0,Math.min(6,Math.round(v/(peak/6))))).slice(27,59);levels.forEach((level,index)=>{$$("#spectrum i")[index]?.style.setProperty("--h",`${8+level*7}px`)});pushLive("/api/live-audio",{levels})},30);$("#lighting-live-label").textContent="AUDIO SYNC";toast("Audio sync started")}catch(error){await stopLive();toast("Could not start audio capture",error.message,true)}});

  const drop=$("#drop-zone"); $("#display-file").addEventListener("change",e=>chooseDisplayFile(e.target.files[0]));
  ["dragenter","dragover"].forEach(type=>drop.addEventListener(type,e=>{e.preventDefault();drop.classList.add("drag")}));
  ["dragleave","drop"].forEach(type=>drop.addEventListener(type,e=>{e.preventDefault();drop.classList.remove("drag")}));
  drop.addEventListener("drop",e=>chooseDisplayFile(e.dataTransfer.files[0]));
  [$("#settings-sync-clock")].filter(Boolean).forEach(button=>button.addEventListener("click",async()=>{try{await withBusy(button,()=>post("/api/clock"));toast("Display clock synced",new Date().toLocaleString("en"))}catch(error){toast("Could not sync clock",error.message,true)}}));

  $$("[data-map-layer]").forEach(button=>button.addEventListener("click",()=>{state.mapLayer=button.dataset.mapLayer;$$("[data-map-layer]").forEach(x=>x.classList.toggle("active",x===button));if(state.selectedKey)selectKey(state.selectedKey)}));
  $("#key-color").addEventListener("input",e=>setKeyColor(e.target.value)); $("#key-color-hex").addEventListener("change",e=>setKeyColor(e.target.value));
  $("#apply-remap").addEventListener("click",async event=>{if(!state.selectedKey)return toast("Select a key first","",true);try{await withBusy(event.currentTarget,()=>post("/api/remap",{source:state.selectedKey,target:$("#remap-target").value||"disabled",modifier:$("#remap-mod").value,second_usage:usageFor($("#remap-second").value),profile:state.profile,fn_layer:state.mapLayer==="fn"?0:null}));toast("Key assignment saved",`${state.selectedKey} → ${$("#remap-target").value||"disabled"}`)}catch(error){toast("Could not assign key",error.message,true)}});
  $("#apply-key-color").addEventListener("click", async event => {
    if (!state.selectedKey) return toast("Select a key first", "", true);
    try {
      await withBusy(event.currentTarget, () => post("/api/key-color", {
        key: state.selectedKey,
        color: $("#key-color").value,
        slot: +$("#key-color-slot").value
      }));
      const color = $("#key-color").value;
      const mapKey = $(`#key-map [data-key="${state.selectedKey}"]`);
      if (mapKey) mapKey.style.setProperty("--rgb", color);
      const rgbKey = $(`#rgb-keyboard [data-key="${state.selectedKey}"]`);
      if (rgbKey) rgbKey.style.setProperty("--rgb", color);
      toast("Key color saved", `${state.selectedKey} · Slot ${+$("#key-color-slot").value + 1}`);
    } catch (error) {
      toast("Could not save key color", error.message, true);
    }
  });

  $("#macro-add").addEventListener("click",()=>{state.macroEvents.push({key:"A",action:"down",delay:60});renderMacroEvents()});
  $("#macro-clear").addEventListener("click",()=>{state.macroEvents=[];renderMacroEvents()});
  $("#macro-record").addEventListener("click",()=>{if(state.recording){stopRecording();return}state.macroEvents=[];state.recording=true;state.lastRecordAt=performance.now();$("#recording-hint").classList.remove("hidden");$("#macro-record").textContent="■ Stop recording";renderMacroEvents();toast("Recording started","Press Esc to finish")});
  document.addEventListener("keydown",event=>handleRecording(event,"down"),true); document.addEventListener("keyup",event=>handleRecording(event,"up"),true);
  $("#apply-macro").addEventListener("click",async event=>{if(!state.macroEvents.length)return toast("Add or record events first","",true);try{await withBusy(event.currentTarget,()=>post("/api/macro",{source:$("#macro-source").value,index:+$("#macro-index").value,repeat_count:+$("#macro-repeat").value,mode:$("#macro-mode").value,profile:state.profile,events:state.macroEvents}));toast("Macro saved",`${state.macroEvents.length} events · Slot ${$("#macro-index").value}`)}catch(error){toast("Could not save macro",error.message,true)}});

  $("#debounce").addEventListener("input",e=>$("#debounce-value").textContent=`${e.target.value} ms`);
  $("#apply-performance").addEventListener("click",async event=>{try{await withBusy(event.currentTarget,async()=>{await post("/api/rate",{hz:+$("#report-rate").value,profile:state.profile});await post("/api/debounce",{milliseconds:+$("#debounce").value})});toast("Performance saved",`${$("#report-rate").value} Hz · ${$("#debounce").value} ms`);await refreshStatus(true)}catch(error){toast("Could not save performance",error.message,true)}});
  $("#apply-options").addEventListener("click",async event=>{const options={profile:state.profile};$$('[data-option]').forEach(input=>options[input.dataset.option]=input.checked);try{await withBusy(event.currentTarget,()=>post("/api/options",options));toast("Keyboard options saved",`Profile ${state.profile+1}`)}catch(error){toast("Could not save options",error.message,true)}});
  $("#apply-sleep").addEventListener("click",async event=>{const payload={bluetooth:+$("#sleep-bt").value,wireless:+$("#sleep-wireless").value,bluetooth_deep:+$("#sleep-bt-deep").value,wireless_deep:+$("#sleep-wireless-deep").value};try{await withBusy(event.currentTarget,()=>post("/api/sleep",payload));toast("Sleep timers saved")}catch(error){toast("Could not save timers",error.message,true)}});
  $("#factory-reset").addEventListener("click",async()=>{if(!confirm("Erase all custom keyboard settings and restore factory defaults?"))return;try{await post("/api/reset");toast("Factory settings restored");setTimeout(()=>refreshStatus(true),1300)}catch(error){toast("Reset failed",error.message,true)}});
  $("#export-config").addEventListener("click",()=>{const blob=new Blob([JSON.stringify({version:1,presets:JSON.parse(localStorage.getItem("k8-presets")||"[]")},null,2)],{type:"application/json"});const link=document.createElement("a");link.href=URL.createObjectURL(blob);link.download="darmoshark-k8-studio.json";link.click();setTimeout(()=>URL.revokeObjectURL(link.href),1000);toast("Presets exported")});
  $("#import-config").addEventListener("change",async event=>{try{const parsed=JSON.parse(await event.target.files[0].text());if(!Array.isArray(parsed.presets))throw new Error("Invalid preset file");localStorage.setItem("k8-presets",JSON.stringify(parsed.presets.slice(-20)));renderPresets();toast("Presets imported",`${parsed.presets.length} presets`)}catch(error){toast("Could not import file",error.message,true)}});

  $("#activity-open").addEventListener("click",()=>openDrawer(true)); $("#activity-close").addEventListener("click",()=>openDrawer(false)); $("#drawer-scrim").addEventListener("click",()=>openDrawer(false));
  $("#activity-clear").addEventListener("click",()=>{state.activities=[];renderActivities()});
  $("#command-open").addEventListener("click",()=>openCommands()); $("#command-modal").addEventListener("click",event=>{if(event.target===$("#command-modal"))openCommands(false)}); $("#command-search").addEventListener("input",renderCommands);
  document.addEventListener("keydown", event => {
    if ((event.ctrlKey || event.metaKey) && ["+", "-", "=", "0"].includes(event.key)) {
      event.preventDefault();
    } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      openCommands();
    } else if (event.key === "Escape" && !state.recording) {
      openCommands(false);
      openDrawer(false);
    } else if (!$("#command-modal").classList.contains("hidden")) {
      const items = $$(".command-result");
      if (!items.length) return;
      let currentIndex = items.findIndex(item => item.classList.contains("active"));
      if (event.key === "ArrowDown") {
        event.preventDefault();
        if (currentIndex >= 0) items[currentIndex].classList.remove("active");
        currentIndex = (currentIndex + 1) % items.length;
        items[currentIndex].classList.add("active");
        items[currentIndex].scrollIntoView({block: "nearest"});
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        if (currentIndex >= 0) items[currentIndex].classList.remove("active");
        currentIndex = (currentIndex - 1 + items.length) % items.length;
        items[currentIndex].classList.add("active");
        items[currentIndex].scrollIntoView({block: "nearest"});
      } else if (event.key === "Enter") {
        event.preventDefault();
        const active = items[currentIndex >= 0 ? currentIndex : 0];
        if (active) active.click();
      }
    }
  });
}

async function boot() {
  Object.entries(EFFECTS).forEach(([value, label]) => {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    $("#effect").append(option);
  });
  renderKeyboard($("#rgb-keyboard"));
  renderKeyboard($("#key-map"), true);
  for (let i = 0; i < 32; i++) {
    const bar = document.createElement("i");
    bar.style.setProperty("--h", "8px");
    $("#spectrum").append(bar);
  }
  renderMacroEvents();
  renderPresets();
  renderActivities();
  wireEvents();
  updateEffectFields();
  await stopLive(false);
  setKeyColor("#ff6432");

  window.addEventListener("hashchange", () => {
    const page = location.hash.replace("#", "");
    if (page && $(`.page[id="${CSS.escape(page)}"]`)) navigate(page);
  });
  const initialPage = location.hash.replace("#", "");
  if (initialPage && $(`.page[id="${CSS.escape(initialPage)}"]`)) navigate(initialPage);
  else navigate("lighting");

  try {
    await resolveBackend();
    if (window.__TAURI__?.core?.invoke) {
      const report = JSON.stringify({
        innerWidth,
        innerHeight,
        devicePixelRatio,
        screenWidth: screen.width,
        sidebar: getComputedStyle($(".sidebar")).width,
        bodyZoom: getComputedStyle(document.body).zoom
      });
      await window.__TAURI__.core.invoke("frontend_report", { report });
    }
    await refreshDevice(true);

  } catch (error) {
    $("#offline-banner").classList.remove("hidden");
    toast("Could not start K8 Studio", error.message, true);
  }
  setInterval(() => refreshDevice(true).catch(() => {}), 15000);
}

boot();
