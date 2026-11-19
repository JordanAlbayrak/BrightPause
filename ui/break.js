import { paint, wantsDark, loadImage, bytesToBase64 } from './backgrounds.js';
import { t, loadStrings } from './i18n.js';

const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
const RADIUS = 74;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

function clock(seconds) {
  const s = Math.max(0, Math.ceil(seconds));
  return s >= 60 ? `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}` : String(s);
}

function lengthText(seconds) {
  return seconds >= 60 ? t('break.minutes', { n: Math.round(seconds / 60) }) : t('break.seconds', { n: Math.round(seconds) });
}

async function drawBackground(bg) {
  const canvas = $('bg');
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(innerWidth * dpr);
  canvas.height = Math.round(innerHeight * dpr);
  let img = null;
  if (bg === 'image') {
    const src = await invoke('get_background_image');
    if (src) img = await loadImage(src).catch(() => null);
  }
  paint(canvas.getContext('2d'), canvas.width, canvas.height, bg, img);

  const small = document.createElement('canvas');
  small.width = 320;
  small.height = 180;
  const ctx = small.getContext('2d');
  if (bg === 'image') ctx.filter = 'blur(3px)';
  ctx.drawImage(canvas, 0, 0, small.width, small.height);
  const pixels = ctx.getImageData(0, 0, small.width, small.height).data;
  invoke('set_side_picture', { width: small.width, height: small.height, rgbaBase64: bytesToBase64(new Uint8Array(pixels.buffer)) }).catch(() => {});
}

async function main() {
  const info = await invoke('get_break');
  if (!info) return;
  await loadStrings();
  const bg = info.background || 'theme';
  if (wantsDark(bg)) document.documentElement.dataset.theme = 'dark';
  else if (info.theme !== 'system') document.documentElement.dataset.theme = info.theme;
  if (bg !== 'theme') document.body.classList.add('on-picture');
  if (bg === 'image') document.body.classList.add('on-photo');
  await drawBackground(bg);

  $('eyebrow').textContent = `${info.label} · ${lengthText(info.length)}`;
  $('title').textContent = info.item.title;
  $('text').textContent = info.item.text;
  if (info.item.source_url) {
    const a = document.createElement('a');
    a.href = info.item.source_url;
    a.target = '_blank';
    a.rel = 'noopener';
    a.textContent = info.item.source_title;
    $('source').append(`${t('break.source')} `, a);
  } else if (info.item.type === 'challenge') {
    $('source').textContent = t('break.challenge');
  }

  const postponeMinutes = info.kind === 'long' ? 5 : 2;
  if (info.can_postpone) {
    $('postpone').hidden = false;
    $('postpone').textContent = t('break.postpone', { n: postponeMinutes });
    $('postpone').addEventListener('click', () => invoke('end_break', { outcome: 'postponed' }));
  }
  if (info.can_skip) {
    $('skip').hidden = false;
    $('skip').addEventListener('click', () => invoke('end_break', { outcome: 'skipped' }));
    $('hint').textContent = t('break.esc');
  } else {
    $('hint').textContent = t('break.strict');
  }
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && info.can_skip) invoke('end_break', { outcome: 'skipped' });
  });

  const bar = $('bar');
  bar.style.strokeDasharray = String(CIRCUMFERENCE);
  const tick = () => {
    const left = (info.ends_at_ms - Date.now()) / 1000;
    $('time').textContent = clock(left);
    bar.style.strokeDashoffset = String(CIRCUMFERENCE * (1 - Math.max(0, left) / info.length));
  };
  tick();
  requestAnimationFrame(() => bar.classList.add('ready'));
  setInterval(tick, 250);
}

main().catch((e) => console.error(e));
