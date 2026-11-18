// Interface strings come from the Rust side (src-tauri/i18n/*.json), in the language picked
// in settings or else the Windows display language.
const { invoke } = window.__TAURI__.core;
let strings = {};
export let lang = 'en';
export let languages = [];

export function t(key, vars = {}) {
  let s = strings[key] ?? key;
  for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, v);
  return s;
}

export async function loadStrings() {
  const r = await invoke('get_strings');
  strings = r.strings;
  lang = r.lang;
  languages = r.languages;
  document.documentElement.lang = lang;
  for (const el of document.querySelectorAll('[data-i18n]')) el.textContent = t(el.dataset.i18n);
  for (const el of document.querySelectorAll('[data-i18n-aria]')) el.setAttribute('aria-label', t(el.dataset.i18nAria));
}
