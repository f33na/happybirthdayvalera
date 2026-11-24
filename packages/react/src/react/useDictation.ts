import { useEffect, useState, type RefObject } from 'react';
import { FieldBinding, type TextField } from '../core/field';
import { isActive } from '../core/machine';
import { useListenerController, type Listener, type UseListenerOptions } from './useListener';

/**
 * `useListener` bound to a `<textarea>` or `<input>`: drafts appear in the field at the caret while the user
 * speaks, each final segment replaces its draft in place, and Esc in the field stops. No glue code is needed,
 * and a controlled field (`value` + `onChange`) sees every change as a normal input event.
 * With `ref = null` it binds nothing and works like `useListener`, so a component can make the field optional.
 */
export function useDictation(ref: RefObject<TextField | null> | null, options: UseListenerOptions = {}): Listener {
  const { listener, controller } = useListenerController(options, ref !== null);
  const [binding] = useState(() => new FieldBinding());

  useEffect(() => {
    const field = ref?.current;
    if (!field) return undefined;
    controller.bind(binding);
    const detach = binding.attach(field, () => {
      if (!isActive(controller.getSnapshot().state)) return false;
      controller.stop();
      return true;
    });
    return () => {
      controller.bind(null);
      detach();
    };
  }, [ref, binding, controller]);

  return listener;
}
