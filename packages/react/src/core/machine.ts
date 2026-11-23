export type ListenerState = 'idle' | 'requesting' | 'loading-model' | 'listening' | 'transcribing' | 'error';

export type ErrorCode = 'mic-denied' | 'mic-unavailable' | 'unsupported' | 'model-failed' | 'transcribe-failed';

export type MachineEvent =
  | { type: 'START' }
  | { type: 'MIC_READY' }
  | { type: 'MIC_FAILED'; error: ErrorCode }
  | { type: 'MODEL_PROGRESS'; fraction: number }
  | { type: 'MODEL_READY' }
  | { type: 'MODEL_FAILED'; error?: ErrorCode }
  | { type: 'STOP' }
  | { type: 'SILENCE' }
  | { type: 'RESULT'; text: string }
  | { type: 'TRANSCRIBE_FAILED' }
  | { type: 'RESET' };

export interface MachineState {
  state: ListenerState;
  error: ErrorCode | null;
  /** Model download progress, 0..1. */
  progress: number;
  /** The last final text, or '' while none. */
  text: string;
  /** True after a RESULT with empty text, until the next START. */
  nothingHeard: boolean;
}

export const initialMachineState: MachineState = {
  state: 'idle',
  error: null,
  progress: 0,
  text: '',
  nothingHeard: false,
};

type Transition = (s: MachineState, e: MachineEvent) => MachineState;

const to =
  (state: ListenerState, patch: Partial<MachineState> = {}) =>
  (s: MachineState): MachineState => ({ ...s, state, ...patch });

// Mirrors `transitions` in shared/machine.json; the table-driven test keeps the two in step.
const table: Partial<Record<ListenerState, Partial<Record<MachineEvent['type'], Transition>>>> = {
  idle: {
    START: to('requesting', { error: null, progress: 0, nothingHeard: false, text: '' }),
  },
  error: {
    START: to('requesting', { error: null, progress: 0, nothingHeard: false, text: '' }),
    RESET: to('idle', { error: null }),
  },
  requesting: {
    MIC_READY: to('loading-model'),
    MIC_FAILED: (s, e) => to('error', { error: e.type === 'MIC_FAILED' ? e.error : 'mic-unavailable' })(s),
    STOP: to('idle'),
  },
  'loading-model': {
    MODEL_PROGRESS: (s, e) => (e.type === 'MODEL_PROGRESS' ? { ...s, progress: clamp01(e.fraction) } : s),
    MODEL_READY: to('listening', { progress: 1 }),
    MODEL_FAILED: (s, e) => to('error', { error: (e.type === 'MODEL_FAILED' && e.error) || 'model-failed' })(s),
    STOP: to('idle'),
  },
  listening: {
    STOP: to('transcribing'),
    SILENCE: to('transcribing'),
  },
  transcribing: {
    RESULT: (s, e) => {
      const text = e.type === 'RESULT' ? e.text.trim() : '';
      return { ...s, state: 'idle', text, nothingHeard: text === '' };
    },
    TRANSCRIBE_FAILED: to('error', { error: 'transcribe-failed' }),
  },
};

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, Number.isFinite(v) ? v : 0));
}

/** Pure reducer. A (state, event) pair outside the table leaves the state unchanged. */
export function reduce(current: MachineState, event: MachineEvent): MachineState {
  const transition = table[current.state]?.[event.type];
  return transition ? transition(current, event) : current;
}

export function isActive(state: ListenerState): boolean {
  return state === 'requesting' || state === 'loading-model' || state === 'listening';
}
