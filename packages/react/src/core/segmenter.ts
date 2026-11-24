/** Absolute sample positions since the start of listening. */
interface Region {
  start: number;
  end: number;
}

export interface SegmenterOptions {
  sampleRate: number;
  thresholdRms: number;
  minSpeechMs: number;
  segmentSilenceMs: number;
  padBeforeMs: number;
  padAfterMs: number;
  maxSegmentMs: number;
  autoStopMs: number;
}

export interface SegmenterBlock {
  pcm: Float32Array;
  rms: number;
  ms: number;
}

export interface SegmenterStep {
  /** The audio of a segment that has just closed, with its padding. */
  closed: Float32Array | null;
  /** True once there has been no speech for `autoStopMs`. */
  autoStop: boolean;
}

/**
 * Splits continuous microphone audio into speech segments at pauses (see CONTRACT.md, "Live dictation").
 * Time is counted in audio blocks, not wall-clock timers, so a test can drive it block by block.
 * It keeps only the audio that the open segment can still need, so memory stays bounded.
 */
export class Segmenter {
  private readonly o: SegmenterOptions;
  private blocks: Float32Array[] = [];
  /** Absolute index of the first sample of `blocks[0]`. */
  private kept = 0;
  /** Absolute index of the next sample. */
  private pos = 0;
  /** Audio before this index belongs to a closed segment. */
  private floor = 0;
  private loudRunMs = 0;
  private loudRunStart = 0;
  private speech: Region | null = null;
  private quietMs = 0;
  private sinceSpeechMs = 0;
  private stopped = false;
  /** Grows by one every time a segment closes; a draft taken from segment N is stale once it changes. */
  seq = 0;

  constructor(options: SegmenterOptions) {
    this.o = options;
  }

  private samples(ms: number): number {
    return Math.round((ms * this.o.sampleRate) / 1000);
  }

  /** True while the open segment has speech. */
  get speaking(): boolean {
    return this.speech !== null;
  }

  private segmentStart(speech: Region): number {
    return Math.max(this.floor, this.kept, speech.start - this.samples(this.o.padBeforeMs));
  }

  push(block: SegmenterBlock): SegmenterStep {
    const at = this.pos;
    this.blocks.push(block.pcm);
    this.pos += block.pcm.length;
    const loud = block.rms >= this.o.thresholdRms;
    let closed: Float32Array | null = null;

    if (!this.speech) {
      if (loud) {
        if (this.loudRunMs === 0) this.loudRunStart = at;
        this.loudRunMs += block.ms;
      } else {
        this.loudRunMs = 0;
      }
      // Several loud blocks in a row are needed; one click or cough must not open a segment.
      if (this.loudRunMs >= this.o.minSpeechMs) {
        this.speech = { start: this.loudRunStart, end: this.pos };
        this.quietMs = 0;
        this.loudRunMs = 0;
      }
    } else if (loud) {
      this.speech.end = this.pos;
      this.quietMs = 0;
    } else {
      this.quietMs += block.ms;
    }

    if (this.speech && loud) {
      this.sinceSpeechMs = 0;
    } else {
      this.sinceSpeechMs += block.ms;
    }

    if (this.speech) {
      if (this.quietMs >= this.o.segmentSilenceMs) {
        closed = this.close(this.speech);
      } else if (this.pos - this.segmentStart(this.speech) >= this.samples(this.o.maxSegmentMs)) {
        // Whisper sees at most 30 s. Cut here; the speech goes on in a new segment that starts at the cut.
        closed = this.slice(this.segmentStart(this.speech), this.pos);
        this.floor = this.pos;
        this.seq += 1;
        this.speech = { start: this.pos, end: this.pos };
        this.quietMs = 0;
      }
    }
    this.trim();

    const autoStop = !this.stopped && this.sinceSpeechMs >= this.o.autoStopMs;
    if (autoStop) this.stopped = true;
    return { closed, autoStop };
  }

  /** Closes the open segment, if it has speech, and returns its audio. For STOP. */
  flush(): Float32Array | null {
    return this.speech ? this.close(this.speech) : null;
  }

  /** The audio of the open segment so far, with the padding before it; null while it has no speech. */
  draft(): Float32Array | null {
    return this.speech ? this.slice(this.segmentStart(this.speech), this.pos) : null;
  }

  private close(speech: Region): Float32Array {
    const start = this.segmentStart(speech);
    const end = Math.min(this.pos, speech.end + this.samples(this.o.padAfterMs));
    const audio = this.slice(start, end);
    this.floor = end;
    this.speech = null;
    this.quietMs = 0;
    this.seq += 1;
    return audio;
  }

  /** Drops whole blocks that no segment can use any more. */
  private trim(): void {
    const need = this.speech
      ? this.segmentStart(this.speech)
      : Math.max(this.floor, (this.loudRunMs > 0 ? this.loudRunStart : this.pos) - this.samples(this.o.padBeforeMs));
    while (this.blocks.length > 0) {
      const first = this.blocks[0];
      if (!first || this.kept + first.length > need) break;
      this.kept += first.length;
      this.blocks.shift();
    }
  }

  private slice(start: number, end: number): Float32Array {
    const out = new Float32Array(Math.max(0, end - start));
    let offset = this.kept;
    for (const block of this.blocks) {
      const from = Math.max(start, offset);
      const to = Math.min(end, offset + block.length);
      if (to > from) out.set(block.subarray(from - offset, to - offset), from - start);
      offset += block.length;
      if (offset >= end) break;
    }
    return out;
  }
}
