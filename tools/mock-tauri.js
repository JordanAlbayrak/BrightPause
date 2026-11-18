(() => {
  const params = new URLSearchParams(location.search);
  const kind = params.get('kind') || 'mini';
  const theme = params.get('theme') || 'system';
  const strict = params.get('strict') === '1';
  const length = kind === 'long' ? 300 : 20;
  const items = {
    mini: { id: 'far-look', kind: 'mini', type: 'message', title: 'Look far away', text: 'Find something about 6 metres (20 feet) away and rest your eyes on it until the timer ends.', source_title: 'American Optometric Association', source_url: 'https://www.aoa.org/healthy-eyes/eye-and-vision-conditions/computer-vision-syndrome' },
    long: { id: 'squats', kind: 'long', type: 'challenge', title: 'Ten slow squats', text: 'Stand up and do ten slow squats or ten calf raises. Hold the desk if you like.', source_title: '', source_url: '' },
  };
  const settings = { mini_every_min: 20, mini_length_sec: 20, long_every_nth: 3, long_length_min: 5, heads_up: true, strict, allow_postpone: true, idle_reset_min: 5, sound: 'chime', volume: 0.5, start_at_login: true, theme, all_screens: true };
  const responses = {
    get_break: () => ({ kind, label: kind === 'long' ? 'Movement break' : 'Eye break', length, ends_at_ms: Date.now() + length * 1000 * 0.65, item: items[kind], can_postpone: !strict, can_skip: !strict, sound: 'none', volume: 0, theme }),
    get_settings: () => ({ ...settings }),
    save_settings: (a) => a.settings,
    get_status: () => ({ text: 'Next: eye break in 14 min', paused: false, stats: { taken: 7, skipped: 1, postponed: 1, streak_days: 4, week: [['Tue', 9], ['Wed', 11], ['Thu', 6], ['Fri', 10], ['Sat', 2], ['Sun', 0], ['Mon', 7]] } }),
    do_action: () => null,
    end_break: () => null,
  };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => responses[cmd](args || {}) } };
})();
