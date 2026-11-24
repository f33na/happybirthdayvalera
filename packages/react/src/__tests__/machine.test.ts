import { describe, expect, it } from 'vitest';
import { machineSpec } from '../core/data';
import { initialMachineState, reduce, type ListenerState, type MachineEvent } from '../core/machine';

const states = machineSpec.states as ListenerState[];

function event(type: string): MachineEvent {
  switch (type) {
    case 'MIC_FAILED':
      return { type, error: 'mic-denied' };
    case 'MODEL_PROGRESS':
      return { type, fraction: 0.4 };
    case 'RESULT':
      return { type, text: 'hello' };
    case 'START':
    case 'MIC_READY':
    case 'MODEL_READY':
    case 'MODEL_FAILED':
    case 'STOP':
    case 'SILENCE':
    case 'TRANSCRIBE_FAILED':
    case 'RESET':
      return { type };
    default:
      throw new Error(`Unknown event ${type}`);
  }
}

const at = (state: ListenerState) => ({ ...initialMachineState, state });

describe('machine.json', () => {
  it('lists only events the reducer knows', () => {
    for (const e of machineSpec.events) expect(() => event(e)).not.toThrow();
  });

  it.each(machineSpec.transitions)('$from + $event -> $to', ({ from, event: name, to }) => {
    expect(reduce(at(from as ListenerState), event(name)).state).toBe(to);
  });

  it.each(machineSpec.ignored)('$from + $event is ignored', ({ from, event: name }) => {
    const before = at(from as ListenerState);
    expect(reduce(before, event(name))).toBe(before);
  });

  it('leaves the state unchanged for every pair outside `transitions`', () => {
    const known = new Set(machineSpec.transitions.map((t) => `${t.from}|${t.event}`));
    for (const from of states) {
      for (const name of machineSpec.events) {
        if (known.has(`${from}|${name}`)) continue;
        const before = at(from);
        expect(reduce(before, event(name)), `${from} + ${name}`).toBe(before);
      }
    }
  });
});

describe('machine data', () => {
  it('records the error code of a failure', () => {
    const failed = reduce(at('requesting'), { type: 'MIC_FAILED', error: 'mic-unavailable' });
    expect(failed.error).toBe('mic-unavailable');
    expect(reduce(at('loading-model'), { type: 'MODEL_FAILED' }).error).toBe('model-failed');
    expect(reduce(at('loading-model'), { type: 'MODEL_FAILED', error: 'unsupported' }).error).toBe('unsupported');
    expect(reduce(at('transcribing'), { type: 'TRANSCRIBE_FAILED' }).error).toBe('transcribe-failed');
  });

  it('tracks progress and clamps it', () => {
    const loading = at('loading-model');
    expect(reduce(loading, { type: 'MODEL_PROGRESS', fraction: 0.4 }).progress).toBe(0.4);
    expect(reduce(loading, { type: 'MODEL_PROGRESS', fraction: 7 }).progress).toBe(1);
  });

  it('sets nothingHeard on an empty RESULT and clears it on START', () => {
    const empty = reduce(at('transcribing'), { type: 'RESULT', text: '  ' });
    expect(empty.state).toBe('idle');
    expect(empty.nothingHeard).toBe(true);
    expect(reduce(empty, { type: 'START' }).nothingHeard).toBe(false);
  });

  it('keeps a trimmed final text', () => {
    expect(reduce(at('transcribing'), { type: 'RESULT', text: ' hi ' }).text).toBe('hi');
  });

  it('clears the error on RESET and on a new START', () => {
    const errored = reduce(at('requesting'), { type: 'MIC_FAILED', error: 'mic-denied' });
    expect(reduce(errored, { type: 'RESET' }).error).toBeNull();
    expect(reduce(errored, { type: 'START' }).error).toBeNull();
  });
});
