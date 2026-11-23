// AudioWorklet of happybirthdayvalera: device audio to 16 kHz mono blocks of 320 samples (20 ms).
// The algorithm mirrors src/resample.rs: a Blackman-windowed sinc low-pass at 90 % of the output Nyquist
// rate when downsampling, then linear interpolation. Each full block goes to the main thread as a
// transferred Float32Array.
const TARGET_RATE = 16000;
const BLOCK = 320;

function lowPass(step) {
  const cutoff = 0.45 / step;
  const half = Math.ceil(8 * step);
  const n = 2 * half + 1;
  const taps = new Float32Array(n);
  let sum = 0;
  for (let i = 0; i < n; i++) {
    const m = i - half;
    const sinc = m === 0 ? 2 * cutoff : Math.sin(2 * Math.PI * cutoff * m) / (Math.PI * m);
    const w = (2 * Math.PI * i) / (n - 1);
    const h = sinc * (0.42 - 0.5 * Math.cos(w) + 0.08 * Math.cos(2 * w));
    taps[i] = h;
    sum += h;
  }
  for (let i = 0; i < n; i++) taps[i] /= sum;
  return taps;
}

class HbvCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.step = sampleRate / TARGET_RATE;
    this.taps = this.step > 1 ? lowPass(this.step) : new Float32Array([1]);
    this.history = new Float32Array(this.taps.length);
    this.head = 0;
    this.prev = 0;
    this.pos = 0;
    this.block = new Float32Array(BLOCK);
    this.fill = 0;
  }

  push(sample) {
    const n = this.taps.length;
    this.history[this.head] = sample;
    this.head = (this.head + 1) % n;
    let filtered = 0;
    for (let i = 0; i < n; i++) filtered += this.history[(this.head + i) % n] * this.taps[i];
    while (this.pos <= 1) {
      this.block[this.fill++] = this.prev + (filtered - this.prev) * this.pos;
      if (this.fill === BLOCK) {
        this.port.postMessage(this.block, [this.block.buffer]);
        this.block = new Float32Array(BLOCK);
        this.fill = 0;
      }
      this.pos += this.step;
    }
    this.pos -= 1;
    this.prev = filtered;
  }

  process(inputs) {
    const input = inputs[0];
    if (!input || input.length === 0) return true;
    const channels = input.length;
    const frames = input[0].length;
    for (let f = 0; f < frames; f++) {
      let mono = 0;
      for (let c = 0; c < channels; c++) mono += input[c][f];
      this.push(mono / channels);
    }
    return true;
  }
}

registerProcessor('hbv-capture', HbvCapture);
