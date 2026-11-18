// Break-screen backgrounds: the theme, a plain colour or the user's own image, painted on a
// canvas so the break page, the setting previews and the side screens all match.

export const COLORS = [
  ['#1f3a2e', 'Pine'],
  ['#173a3d', 'Teal'],
  ['#1d2a3a', 'Slate'],
  ['#25284a', 'Indigo'],
  ['#33243f', 'Plum'],
  ['#3d1f2a', 'Wine'],
  ['#3d2c22', 'Walnut'],
  ['#2a2a2a', 'Graphite'],
];

function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function glow(ctx, w, h, [color, x, y, r, alpha]) {
  const g = ctx.createRadialGradient(x * w, y * h, 0, x * w, y * h, r * Math.max(w, h));
  ctx.globalAlpha = alpha;
  g.addColorStop(0, color);
  g.addColorStop(1, 'transparent');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
  ctx.globalAlpha = 1;
}

function scrim(ctx, w, h, strength) {
  const g = ctx.createRadialGradient(w / 2, h * 0.45, 0, w / 2, h * 0.45, Math.max(w, h) * 0.6);
  g.addColorStop(0, `rgba(6, 10, 8, ${strength})`);
  g.addColorStop(1, `rgba(6, 10, 8, ${Math.min(0.85, strength + 0.2)})`);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
}

function cover(ctx, w, h, img) {
  const s = Math.max(w / img.naturalWidth, h / img.naturalHeight);
  const iw = img.naturalWidth * s, ih = img.naturalHeight * s;
  ctx.drawImage(img, (w - iw) / 2, (h - ih) / 2, iw, ih);
}

// True when the page should use light text (the dark theme tokens).
export function wantsDark(bg) {
  return bg !== 'theme';
}

export function paint(ctx, w, h, bg, img) {
  ctx.clearRect(0, 0, w, h);
  if (bg.startsWith('color:')) {
    ctx.fillStyle = bg.slice(6);
    ctx.fillRect(0, 0, w, h);
  } else if (bg === 'image' && img) {
    cover(ctx, w, h, img);
    scrim(ctx, w, h, 0.5);
  } else if (bg === 'image') {
    ctx.fillStyle = '#151a17';
    ctx.fillRect(0, 0, w, h);
  } else {
    ctx.fillStyle = cssVar('--bg');
    ctx.fillRect(0, 0, w, h);
    glow(ctx, w, h, [cssVar('--accent-soft'), 0.5, 0.38, 0.45, 1]);
  }
}

export function loadImage(src) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

export function bytesToBase64(bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(s);
}
