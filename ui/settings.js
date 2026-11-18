import { play } from './sound.js';
import { COLORS, paint, loadImage } from './backgrounds.js';

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
  if (unit === 'sec') return `${value} sec`;
  return value === 1 ? '1 min' : `${value} min`;
}

function applyTheme() {
  if (settings.theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = settings.theme;
}

function cadence() {
  const n = settings.long_every_nth;
  const every = settings.mini_every_min * n;
  const hours = every === 60 ? 'hour' : every % 60 === 0 ? `${every / 60} hours` : `${every} minutes`;
  $('cadence').textContent = `So: a ${settings.mini_length_sec}-second eye break every ${settings.mini_every_min} minutes, and a ${settings.long_length_min}-minute movement break about every ${hours}.`;
}

function render() {
  for (const el of document.querySelectorAll('.stepper')) {
    el.querySelector('output').textContent = unitText(settings[el.dataset.key], el.dataset.unit);
  }
  for (const el of document.querySelectorAll('input[type="checkbox"][data-key]')) el.checked = settings[el.dataset.key];
  for (const el of document.querySelectorAll('input[type="radio"]')) el.checked = String(settings[el.name]) === el.value;
  document.querySelector('input[data-key="volume"]').value = settings.volume;
  document.querySelector('input[data-key="allow_postpone"]').disabled = settings.strict;
  applyTheme();
  cadence();
}

function save() {
  saving = saving.then(async () => {
    try {
      settings = await invoke('save_settings', { settings });
      render();
      toast('Saved');
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
    minus.setAttribute('aria-label', `Less ${key.replace(/_/g, ' ')}`);
    plus.setAttribute('aria-label', `More ${key.replace(/_/g, ' ')}`);
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
    ['theme', 'Theme'],
    ...COLORS.map(([c, name]) => [`color:${c}`, name]),
    ['image', img ? 'Your image' : 'Add image'],
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
    toast('That image could not be used.', true);
  }
}

function when(seconds) {
  return new Date(seconds * 1000).toLocaleString([], { weekday: 'short', hour: 'numeric', minute: '2-digit' });
}

function renderUpdate(u) {
  $('update-available').hidden = u.state !== 'available';
  if (u.state === 'available') {
    const dot = document.createElement('span');
    dot.className = 'update-dot';
    $('update-title').replaceChildren(dot, `Version ${u.version} available`);
    $('update-notes').textContent = u.notes || 'A newer BrightPause is ready to install.';
  }
  const text = {
    checking: 'Checking...',
    installing: 'Installing. BrightPause will restart.',
    none: `You have the latest version (checked ${when(u.checked_at)}).`,
    error: `Could not check: ${u.message}`,
    unconfigured: u.message,
  }[u.state] || (u.checked_at ? `Last checked ${when(u.checked_at)}.` : 'BrightPause never goes online unless you ask it to.');
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
  if (st.stats.skipped) parts.push(`${st.stats.skipped} skipped`);
  if (st.stats.postponed) parts.push(`${st.stats.postponed} postponed`);
  if (st.stats.streak_days > 1) parts.push(`${st.stats.streak_days}-day streak`);
  $('today-sub').textContent = parts.join(' · ') || (st.stats.taken ? 'Nicely done.' : 'Your first break is on its way.');
  const max = Math.max(1, ...st.stats.week.map(([, n]) => n));
  $('week').replaceChildren(...st.stats.week.map(([day, n]) => {
    const col = document.createElement('div');
    const bar = document.createElement('i');
    bar.style.height = `${Math.round((n / max) * 44) + 3}px`;
    col.title = `${day}: ${n}`;
    col.append(bar, day.slice(0, 2));
    return col;
  }));
}

async function main() {
  buildSteppers();
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
  setInterval(refresh, 5000);
}

main().catch((e) => toast(String(e), true));
