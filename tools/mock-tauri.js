(() => {
  const params = new URLSearchParams(location.search);
  const kind = params.get('kind') || 'mini';
  const theme = params.get('theme') || 'system';
  const strict = params.get('strict') === '1';
  const lang = params.get('lang') || 'en';
  const length = kind === 'long' ? 300 : 20;
  const items = {
    mini: { id: 'far-look', kind: 'mini', type: 'message', title: 'Look far away', text: 'Find something about 6 metres away and rest your eyes on it until the timer ends.', source_title: 'American Optometric Association', source_url: 'https://www.aoa.org/healthy-eyes/eye-and-vision-conditions/computer-vision-syndrome' },
    long: { id: 'squats', kind: 'long', type: 'challenge', title: 'Ten slow squats', text: 'Stand up and do ten slow squats or ten calf raises. Hold the desk if you like.', source_title: '', source_url: '' },
  };
  const settings = { mini_every_min: 20, mini_length_sec: 20, long_every_nth: 3, long_length_min: 5, heads_up: true, strict, allow_postpone: true, idle_reset_min: 5, sound: 'chime', volume: 0.5, start_at_login: true, theme, all_screens: true, background: 'theme', auto_update: false, language: lang, pause_in_calls: true, shortcuts: { break_now: { keys: 'Ctrl+Alt+B', on: true }, pause: { keys: 'Ctrl+Alt+P', on: true }, skip: { keys: 'Ctrl+Alt+S', on: true } }, my_texts: { short: ['Refill your water glass', 'Look at something green'], long: ['Walk to the mailbox and back'], only_mine: false } };
  // Strings copied next to this file by tools/shots.ps1.
  const xhr = new XMLHttpRequest();
  xhr.open('GET', `i18n-${lang}.json`, false);
  xhr.send();
  const strings = JSON.parse(xhr.responseText);
  for (const item of Object.values(items)) Object.assign(item, (strings.tips || {})[item.id] || {});
  const fill = (key, vars) => Object.entries(vars).reduce((s, [k, v]) => s.replace(`{${k}}`, v), strings[key]);
  const day = (n) => new Date(Date.now() - n * 86400000).toISOString().slice(0, 10);
  const responses = {
    get_strings: () => ({ lang, strings, languages: [['en', 'English'], ['fr', 'Français'], ['es', 'Español'], ['de', 'Deutsch']] }),
    take_welcome: () => params.get('welcome') === '1',
    suspend_hotkeys: () => null,
    resume_hotkeys: () => null,
    get_background_image: () => null,
    set_background_image: () => null,
    set_side_picture: () => null,
    check_updates: () => null,
    play_sound: () => null,
    install_update: () => null,
    get_break: () => ({ kind, label: strings[kind === 'long' ? 'kind.long' : 'kind.eye'], length, ends_at_ms: Date.now() + length * 1000 * 0.65, item: items[kind], can_postpone: !strict, can_skip: !strict, sound: 'none', volume: 0, theme, background: 'theme', lang }),
    get_settings: () => ({ ...settings }),
    save_settings: (a) => a.settings,
    get_status: () => ({ text: fill('status.next_mini', { when: fill('status.in_min', { n: 14 }) }), paused: false, stats: { taken: 7, skipped: 1, postponed: 1, streak_days: 4, week: [9, 11, 6, 10, 2, 0, 7].map((n, i) => [day(6 - i), n]) }, update: { state: params.get('update') || 'idle', version: '', notes: '', message: '', checked_at: 0 } }),
    do_action: () => null,
    end_break: () => null,
  };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => responses[cmd](args || {}) }, app: { getVersion: async () => 'mock' } };
})();
