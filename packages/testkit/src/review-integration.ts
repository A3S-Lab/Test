import { useRef } from "react";

export type ReviewCallback<T> = (value: T) => void | Promise<void>;

export function useLatest<T>(value: T) {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}

export function isEditableEvent(event: KeyboardEvent): boolean {
  const target = event.composedPath()[0];
  if (!(target instanceof Element)) return false;
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  if (target.closest("[contenteditable]:not([contenteditable='false'])")) return true;
  const role = target.getAttribute("role") ?? target.closest("[role]")?.getAttribute("role");
  return role === "textbox" || role === "searchbox" || role === "combobox" || role === "spinbutton";
}

export function invokeCallback<T>(callback: ReviewCallback<T> | undefined, value: T): void {
  try {
    const result = callback?.(value);
    if (result) void result.catch(() => undefined);
  } catch {
    // Host integration failures must not break reviewer state.
  }
}

export async function writeClipboard(
  text: string,
  adapter: ((text: string) => void | Promise<void>) | undefined,
): Promise<boolean> {
  try {
    const writer = adapter ?? navigator.clipboard?.writeText?.bind(navigator.clipboard);
    if (!writer) return false;
    await writer(text);
    return true;
  } catch {
    return false;
  }
}
