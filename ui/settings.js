import { play } from './sound.js';
import { COLORS, paint, loadImage } from './backgrounds.js';
import { t, lang, languages, loadStrings } from './i18n.js';

const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
let settings = null;
let saving = Promise.resolve();
let toastTimer = null;

function toast(text, error = false) {
  const t = $('toast');
  t.textContent = text;
  t.classList.toggle('error', error);
  t.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove('show'), error ? 4000 : 1200);
}

function unitText(value, unit) {
  return t(unit === 'sec' ? 'unit.sec' : 'unit.min', { n: value });
}

function applyTheme() {
  if (settings.theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = settings.theme;
}

function cadence() {
  const n = settings.long_every_nth;
  const every = settings.mini_every_min * n;
  const vars = { sec: settings.mini_length_sec, every: settings.mini_every_min, len: settings.long_length_min };
  if (every === 60) $('cadence').textContent = t('cadence.hour', vars);
  else if (every % 60 === 0) $('cadence').textContent = t('cadence.hours', { ...vars, n: every / 60 });
  else $('cadence').textContent = t('cadence.minutes', { ...vars, n: every });
}

function render() {
  for (const el of document.querySelectorAll('.stepper')) {
    el.querySelector('output').textContent = unitText(settings[el.dataset.key], el.dataset.unit);
  }
  for (const el of document.querySelectorAll('input[type="checkbox"][data-key]')) el.checked = settings[el.dataset.key];
  for (const el of document.querySelectorAll('input[type="radio"]')) el.checked = String(settings[el.name]) === el.value;
  document.querySelector('input[data-key="volume"]').value = settings.volume;
  $('language').value = settings.language;
  document.querySelector('input[data-key="allow_postpone"]').disabled = settings.strict;
  applyTheme();
  cadence();
  renderHotkeys();
}

let recording = null;

function keysLabel(keys) {
  const names = { Ctrl: t('key.ctrl'), Shift: t('key.shift'), Alt: t('key.alt') };
  return keys.split('+').map((k) => names[k] ?? k).join('+');
}

function renderHotkeys() {
  for (const b of document.querySelectorAll('[data-hotkey]')) {
    if (b !== recording) b.textContent = keysLabel(settings.shortcuts[b.dataset.hotkey].keys);
  }
  for (const c of document.querySelectorAll('[data-hotkey-on]')) c.checked = settings.shortcuts[c.dataset.hotkeyOn].on;
}

function clashes(name, keys) {
  return Object.entries(settings.shortcuts).some(([n, h]) => n !== name && h.on && h.keys.toLowerCase() === keys.toLowerCase());
}

function stopRecording(saved) {
  recording.classList.remove('recording');
  recording = null;
  renderHotkeys();
  if (!saved) invoke('resume_hotkeys');
}

function buildHotkeys() {
  for (const b of document.querySelectorAll('[data-hotkey]')) {
    b.addEventListener('click', async () => {
      if (recording) stopRecording(false);
      await invoke('suspend_hotkeys');
      recording = b;
      b.classList.add('recording');
      b.textContent = t('short.press');
    });
  }
  for (const c of document.querySelectorAll('[data-hotkey-on]')) {
    c.addEventListener('change', () => {
      const name = c.dataset.hotkeyOn;
      if (c.checked && clashes(name, settings.shortcuts[name].keys)) {
        c.checked = false;
        toast(t('short.duplicate', { keys: keysLabel(settings.shortcuts[name].keys) }), true);
        return;
      }
      settings.shortcuts[name].on = c.checked;
      save();
    });
  }
  document.addEventListener('keydown', (e) => {
    if (!recording) return;
    e.preventDefault();
    if (e.key === 'Escape') return stopRecording(false);
    if (['Control', 'Alt', 'Shift', 'Meta', 'OS'].includes(e.key)) return;
    const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean);
    if (!mods.length) return toast(t('short.need_modifier'), true);
    const keys = [...mods, e.code.replace(/^Key|^Digit/, '')].join('+');
    const name = recording.dataset.hotkey;
    if (clashes(name, keys)) return toast(t('short.duplicate', { keys: keysLabel(keys) }), true);
    settings.shortcuts[name].keys = keys;
    settings.shortcuts[name].on = true;
    stopRecording(true);
    save();
  });
}

function save() {
  saving = saving.then(async () => {
    try {
      settings = await invoke('save_settings', { settings });
      render();
      toast(t('toast.saved'));
      refresh();
    } catch (e) {
      toast(String(e), true);
      settings = await invoke('get_settings');
      render();
    }
  });
}

function buildSteppers() {
  for (const el of document.querySelectorAll('.stepper')) {
    const { key, unit } = el.dataset;
    const min = Number(el.dataset.min), max = Number(el.dataset.max), step = Number(el.dataset.step);
    const minus = document.createElement('button');
    const plus = document.createElement('button');
    const out = document.createElement('output');
    minus.textContent = '−';
    plus.textContent = '+';
    minus.setAttribute('aria-label', t('aria.less'));
    plus.setAttribute('aria-label', t('aria.more'));
    const change = (dir) => {
      const v = settings[key];
      const next = dir > 0 ? Math.min(max, (Math.floor(v / step) + 1) * step) : Math.max(min, (Math.ceil(v / step) - 1) * step);
      if (next !== v) {
        settings[key] = next;
        render();
        save();
      }
    };
    minus.addEventListener('click', () => change(-1));
    plus.addEventListener('click', () => change(1));
    el.append(minus, out, plus);
    void unit;
  }
}

async function storedImage() {
  const src = await invoke('get_background_image');
  return src ? loadImage(src).catch(() => null) : null;
}

async function buildTiles() {
  const img = await storedImage();
  const options = [
    ['theme', t('tile.theme')],
    ...COLORS.map(([c, name]) => [`color:${c}`, t(`color.${name.toLowerCase()}`)]),
    ['image', t(img ? 'tile.your_image' : 'tile.add_image')],
  ];
  $('tiles').replaceChildren(...options.map(([value, name]) => {
    const tile = document.createElement('label');
    tile.className = 'tile';
    const input = document.createElement('input');
    input.type = 'radio';
    input.name = 'background';
    input.value = value;
    tile.append(input);
    if (value === 'image' && !img) {
      const add = document.createElement('div');
      add.className = 'add';
      add.textContent = '+';
      tile.append(add);
    } else {
      const c = document.createElement('canvas');
      c.width = 192;
      c.height = 108;
      paint(c.getContext('2d'), c.width, c.height, value, img);
      tile.append(c);
    }
    const caption = document.createElement('span');
    caption.textContent = name;
    tile.append(caption);
    if (value === 'image') input.addEventListener('click', pickImage);
    else input.addEventListener('change', () => { settings.background = value; render(); save(); });
    return tile;
  }));
  render();
}

function readFile(file) {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result);
    r.onerror = reject;
    r.readAsDataURL(file);
  });
}

async function pickImage() {
  render();
  const input = $('image-file');
  input.value = '';
  const file = await new Promise((resolve) => { input.onchange = () => resolve(input.files[0]); input.click(); });
  if (!file) return;
  try {
    const img = await loadImage(await readFile(file));
    const s = Math.min(1, 2560 / img.naturalWidth, 1440 / img.naturalHeight);
    const c = document.createElement('canvas');
    c.width = Math.round(img.naturalWidth * s);
    c.height = Math.round(img.naturalHeight * s);
    c.getContext('2d').drawImage(img, 0, 0, c.width, c.height);
    await invoke('set_background_image', { jpegBase64: c.toDataURL('image/jpeg', 0.86).split(',')[1] });
    settings.background = 'image';
    await buildTiles();
    save();
  } catch (e) {
    toast(t('err.image'), true);
  }
}

function when(seconds) {
  return new Date(seconds * 1000).toLocaleString(lang, { weekday: 'short', hour: 'numeric', minute: '2-digit' });
}

function renderUpdate(u) {
  $('update-available').hidden = u.state !== 'available';
  if (u.state === 'available') {
    const dot = document.createElement('span');
    dot.className = 'update-dot';
    $('update-title').replaceChildren(dot, t('update.available', { v: u.version }));
    $('update-notes').textContent = u.notes || t('update.notes_default');
  }
  const text = {
    checking: t('update.checking'),
    installing: t('update.installing'),
    none: t('update.latest', { when: when(u.checked_at) }),
    error: t('update.error', { msg: u.message }),
    unconfigured: t('update.unconfigured'),
  }[u.state] || (u.checked_at ? t('update.last', { when: when(u.checked_at) }) : t('update.offline'));
  $('update-status').textContent = text;
  $('check-updates').disabled = u.state === 'checking' || u.state === 'installing';
  if (u.state === 'checking' || u.state === 'installing') setTimeout(refresh, 800);
}

async function refresh() {
  const st = await invoke('get_status');
  renderUpdate(st.update);
  $('status').textContent = st.text;
  $('pause').hidden = st.paused;
  $('resume').hidden = !st.paused;
  $('taken').textContent = st.stats.taken;
  const parts = [];
  if (st.stats.skipped) parts.push(t('today.skipped', { n: st.stats.skipped }));
  if (st.stats.postponed) parts.push(t('today.postponed', { n: st.stats.postponed }));
  if (st.stats.streak_days > 1) parts.push(t('today.streak', { n: st.stats.streak_days }));
  $('today-sub').textContent = parts.join(' · ') || t(st.stats.taken ? 'today.nice' : 'today.first');
  const max = Math.max(1, ...st.stats.week.map(([, n]) => n));
  $('week').replaceChildren(...st.stats.week.map(([day, n]) => {
    const col = document.createElement('div');
    const bar = document.createElement('i');
    bar.style.height = `${Math.round((n / max) * 44) + 3}px`;
    const [y, m, d] = day.split('-').map(Number);
    const name = new Date(y, m - 1, d).toLocaleDateString(lang, { weekday: 'short' }).replace('.', '');
    col.title = `${name}: ${n}`;
    col.append(bar, name.charAt(0).toUpperCase() + name.slice(1, 2));
    return col;
  }));
}

function buildLanguages() {
  const sel = $('language');
  sel.replaceChildren(...[['system', t('lang.system')], ...languages].map(([value, name]) => new Option(name, value)));
  sel.addEventListener('change', async () => {
    settings.language = sel.value;
    save();
    await saving;
    location.reload();
  });
}

async function main() {
  await loadStrings();
  if (await invoke('take_welcome')) {
    $('welcome').hidden = false;
    requestAnimationFrame(() => scrollTo(0, 0));
    $('welcome-ok').addEventListener('click', () => { $('welcome').hidden = true; });
  }
  buildLanguages();
  buildSteppers();
  buildHotkeys();
  settings = await invoke('get_settings');
  render();
  requestAnimationFrame(() => document.body.classList.add('ready'));
  for (const el of document.querySelectorAll('input[type="checkbox"][data-key]')) {
    el.addEventListener('change', () => { settings[el.dataset.key] = el.checked; render(); save(); });
  }
  $('version').textContent = `BrightPause ${await window.__TAURI__.app.getVersion()}`;
  $('check-updates').addEventListener('click', async () => { await invoke('check_updates'); refresh(); });
  $('install-update').addEventListener('click', async () => {
    try { await invoke('install_update'); } catch (e) { toast(String(e), true); }
    refresh();
  });
  for (const el of document.querySelectorAll('input[type="radio"]:not([name="background"])')) {
    el.addEventListener('change', () => {
      settings[el.name] = el.name === 'long_every_nth' ? Number(el.value) : el.value;
      render();
      save();
      if (el.name === 'sound') play(settings.sound, settings.volume);
    });
  }
  const vol = document.querySelector('input[data-key="volume"]');
  vol.addEventListener('change', () => { settings.volume = Number(vol.value); save(); play(settings.sound, settings.volume); });
  $('test-sound').addEventListener('click', () => play(settings.sound, settings.volume));
  for (const el of document.querySelectorAll('[data-action]')) {
    el.addEventListener('click', async () => { await invoke('do_action', { id: el.dataset.action }); refresh(); });
  }
  await refresh();
  await buildTiles();
  if (!$('welcome').hidden) scrollTo(0, 0);
  setInterval(refresh, 5000);
}

main().catch((e) => toast(String(e), true));
