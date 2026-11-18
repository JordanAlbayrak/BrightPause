const SOUNDS = {
  chime: [[660, 0], [880, 0.18]],
  bell: [[523, 0]],
  soft: [[392, 0], [494, 0.25]],
};

export function play(name, volume) {
  const notes = SOUNDS[name];
  if (!notes || volume <= 0) return;
  try {
    const ctx = new AudioContext();
    for (const [freq, at] of notes) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = name === 'soft' ? 'triangle' : 'sine';
      osc.frequency.value = freq;
      const start = ctx.currentTime + at;
      const tail = name === 'bell' ? 2.2 : 1.1;
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.exponentialRampToValueAtTime(0.25 * volume, start + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + tail);
      osc.connect(gain).connect(ctx.destination);
      osc.start(start);
      osc.stop(start + tail + 0.05);
    }
  } catch (e) {
    console.warn('no audio', e);
  }
}
