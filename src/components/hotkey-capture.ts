import type { HotkeyView } from "../ipc";

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph"]);

export function formatHotkey(hotkey: HotkeyView | null): string {
  if (!hotkey || !hotkey.key) return "";
  const parts: string[] = [];
  if (hotkey.ctrl) parts.push("Ctrl");
  if (hotkey.alt) parts.push("Alt");
  if (hotkey.shift) parts.push("Shift");
  if (hotkey.superKey) parts.push("Super");
  parts.push(hotkey.key.length === 1 ? hotkey.key.toUpperCase() : hotkey.key);
  return parts.join("+");
}

export interface HotkeyCaptureOptions {
  initial: HotkeyView | null;
  placeholder?: string;
  compact?: boolean;
  onChange(hotkey: HotkeyView | null): void;
}

export function createHotkeyCapture(options: HotkeyCaptureOptions): HTMLElement {
  let current = options.initial;

  const el = document.createElement("button");
  el.type = "button";
  el.className = "hotkey-capture";
  if (options.compact) el.classList.add("compact");
  el.title = "Clic para asignar · Retroceso borra · Esc cancela";

  const idle = () => {
    const text = formatHotkey(current);
    el.textContent = text || (options.placeholder ?? "Sin atajo");
    el.classList.toggle("empty", !text);
  };

  const stop = () => {
    el.classList.remove("capturing");
    el.removeEventListener("keydown", onKeyDown);
    idle();
    el.blur();
  };

  const onKeyDown = (event: KeyboardEvent) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      stop();
      return;
    }
    if (event.key === "Backspace") {
      current = null;
      options.onChange(null);
      stop();
      return;
    }
    if (MODIFIER_KEYS.has(event.key)) return;

    current = {
      ctrl: event.ctrlKey,
      alt: event.altKey,
      shift: event.shiftKey,
      superKey: event.metaKey,
      key: event.key.length === 1 ? event.key.toUpperCase() : event.key,
    };
    options.onChange(current);
    stop();
  };

  el.addEventListener("click", (event) => {
    event.stopPropagation();
    if (el.classList.contains("capturing")) return;
    el.classList.add("capturing");
    el.textContent = "Presiona una combinación…";
    el.addEventListener("keydown", onKeyDown);
    el.focus();
  });

  el.addEventListener("blur", () => {
    if (el.classList.contains("capturing")) stop();
  });

  idle();
  return el;
}
