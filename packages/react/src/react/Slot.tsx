import { cloneElement, isValidElement, version, type ReactElement, type ReactNode, type Ref } from 'react';

type AnyProps = Record<string, unknown>;

function setRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (typeof ref === 'function') ref(value);
  else if (ref) ref.current = value;
}

export function mergeRefs<T>(...refs: (Ref<T> | undefined)[]): (node: T | null) => void {
  return (node) => refs.forEach((ref) => setRef(ref, node));
}

/** React 19 passes `ref` as a prop; React 18 keeps it on the element. Reading `element.ref` in 19 warns. */
const REF_IS_PROP = Number.parseInt(version, 10) >= 19;

function childRef(element: ReactElement): Ref<unknown> | undefined {
  // `ref` is not in the element's typed props in either version; the check above says where it lives.
  const props = element.props as AnyProps;
  return (REF_IS_PROP ? props.ref : (element as unknown as { ref?: Ref<unknown> }).ref) as Ref<unknown> | undefined;
}

const isHandler = (key: string): boolean => /^on[A-Z]/.test(key);

/**
 * Merges the props of a slot with the props of its only child:
 * `className` and `style` are combined, event handlers are composed (the child's runs first and can
 * `preventDefault()` to skip ours), and for every other prop the slot wins, so aria and data attributes stay.
 */
export function mergeProps(slot: AnyProps, child: AnyProps): AnyProps {
  const merged: AnyProps = { ...child };
  for (const [key, value] of Object.entries(slot)) {
    if (value === undefined) continue;
    const own = child[key];
    if (key === 'className') {
      merged.className = [value, own].filter(Boolean).join(' ');
    } else if (key === 'style') {
      merged.style = { ...(value as object), ...(own as object | undefined) };
    } else if (isHandler(key) && typeof value === 'function' && typeof own === 'function') {
      merged[key] = (event: { defaultPrevented?: boolean }) => {
        own(event);
        if (!event?.defaultPrevented) value(event);
      };
    } else {
      merged[key] = value;
    }
  }
  return merged;
}

export interface SlotProps extends AnyProps {
  children?: ReactNode;
  /** Rendered inside the child, before the child's own children. */
  inner?: ReactNode;
  /** Not named `ref`: React 18 would strip it from the props of a function component. */
  slotRef?: Ref<unknown>;
}

/** A minimal Slot: renders its single child element with the merged props, instead of an element of its own. */
export function Slot({ children, inner, slotRef, ...slotProps }: SlotProps) {
  if (!isValidElement(children)) {
    throw new Error('asChild needs exactly one React element as its child');
  }
  const element = children as ReactElement<AnyProps>;
  const merged = mergeProps(slotProps, element.props);
  const refs = mergeRefs(slotRef, childRef(element));
  return cloneElement(element, { ...merged, ref: refs }, inner, element.props.children as ReactNode);
}
