// The waveform and mic-level animator of happybirthdayvalera, run inside the page with `document::eval`.
//
// Rust prepends `const HBV_ID = …; const HBV_TOK = …;` (the listener id and the `wave` tokens) and appends
// the call to `hbvRun`. Rust sends `{type:'state', state}`, `{type:'level', rms}` (one per 20 ms block while
// listening), `{type:'celebrate', on}`, `{type:'kick'}` (an element mounted) and `{type:'stop'}`. Each animation frame smooths the
// level and redraws every `svg.hbv-wave` and `.hbv-mic` of this listener, so no Rust render and no IPC
// message happens per frame. The math is the contract's formula; src/wave.rs is the Rust twin, and a test
// checks both against shared/wave-fixture.json.

function hbvFmt(v) {
  const s = (Math.round(v * 100) / 100).toFixed(2);
  return s === '-0.00' ? '0.00' : s;
}

function hbvStrandPath(tok, i, t, amp) {
  const s = tok.strands[i];
  const W = tok.viewBoxWidth;
  const H = tok.viewBoxHeight;
  const n = Math.max(tok.points, 2);
  const parts = [];
  for (let k = 0; k < n; k++) {
    const x = k / (n - 1);
    const env = Math.sin(Math.PI * x) ** 2;
    const y = H / 2 + (H / 2 - tok.strokePad) * env * s.amp * amp * Math.sin(2 * Math.PI * s.freq * x + s.phase + s.speed * t);
    parts.push(`${k === 0 ? 'M' : 'L'}${hbvFmt(x * W)} ${hbvFmt(y)}`);
  }
  return parts.join(' ');
}

function hbvSmooth(tok, level, rms) {
  const target = Math.min(Math.max(rms * tok.levelGain, 0), 1);
  return level + (target - level) * (target > level ? tok.attack : tok.release);
}

// The level the waveform uses while the easter egg plays (`CELEBRATE_LEVEL` in src/wave.rs).
const HBV_CELEBRATE_LEVEL = 0.6;

function hbvAmplitude(tok, state, level, celebrating, reduced) {
  if (reduced) return tok.reducedMotionAmp;
  if (celebrating) return tok.idleAmp + (1 - tok.idleAmp) * HBV_CELEBRATE_LEVEL;
  return state === 'listening' ? tok.idleAmp + (1 - tok.idleAmp) * level : tok.restAmp;
}

async function hbvRun(dioxus, id, tok) {
  const media = typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;
  const scope = `[data-hbv-listener="${id}"]`;
  const mountedAt = new WeakMap(); // svg → time of the first frame that saw it
  const drawnFor = new WeakMap(); // svg → state drawn under reduced motion
  let state = 'idle';
  let rms = 0;
  let celebrating = false;
  let level = 0;
  let shownLevel = -1;
  let raf = 0;
  let stopped = false;

  const schedule = () => {
    if (!raf && !stopped) raf = requestAnimationFrame(frame);
  };

  function frame(now) {
    raf = 0;
    if (stopped) return;
    const reduced = !!(media && media.matches);
    level = hbvSmooth(tok, level, state === 'listening' ? rms : 0);
    const rounded = Math.round(level * 1000) / 1000;

    const mics = document.querySelectorAll(`.hbv-mic${scope}`);
    if (rounded !== shownLevel) {
      for (const mic of mics) mic.style.setProperty('--hbv-level', String(rounded));
      shownLevel = rounded;
    }

    let pending = false;
    const waves = document.querySelectorAll(`svg.hbv-wave${scope}`);
    for (const svg of waves) {
      if (!mountedAt.has(svg)) mountedAt.set(svg, now);
      const drawKey = `${state}:${celebrating}`;
      if (reduced && drawnFor.get(svg) === drawKey) continue;
      const t = reduced ? 0 : (now - mountedAt.get(svg)) / 1000;
      const amp = hbvAmplitude(tok, state, level, celebrating, reduced);
      for (const path of svg.querySelectorAll('.hbv-wave__strand')) {
        const i = Number(path.getAttribute('data-i'));
        if (i >= 0 && i < tok.strands.length) path.setAttribute('d', hbvStrandPath(tok, i, t, amp));
      }
      if (reduced) drawnFor.set(svg, drawKey);
      else {
        drawnFor.delete(svg);
        pending = true; // time moves on, so the next frame differs
      }
    }
    // The mic level needs frames while it moves; an idle widget without a waveform costs nothing.
    if (pending || (mics.length > 0 && (state === 'listening' || rounded > 0))) schedule();
  }

  if (media && typeof media.addEventListener === 'function') media.addEventListener('change', schedule);
  schedule();
  while (!stopped) {
    let msg;
    try {
      msg = await dioxus.recv();
    } catch (_) {
      break;
    }
    if (!msg) continue;
    if (msg.type === 'stop') stopped = true;
    else if (msg.type === 'state') {
      state = msg.state;
      if (state !== 'listening') rms = 0;
    } else if (msg.type === 'level') rms = msg.rms;
    else if (msg.type === 'celebrate') celebrating = !!msg.on;
    schedule();
  }
  if (raf) cancelAnimationFrame(raf);
  if (media && typeof media.removeEventListener === 'function') media.removeEventListener('change', schedule);
}
