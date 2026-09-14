import { type HotkeyView, ipc } from "../ipc";

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph"]);

export function formatHotkey(hotkey: HotkeyView | null): string {
  if (!hotkey?.key) return "";
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
  ariaLabel?: string;
  onChange(hotkey: HotkeyView | null): void;
}

export interface HotkeyCapture {
  element: HTMLElement;
  /** Updates the displayed value. Ignored while the widget is capturing. */
  setValue(hotkey: HotkeyView | null): void;
}

// Global hotkeys are released while any capture widget is active so the
// pressed combination reaches the widget instead of switching windows.
// Reference-counted because several widgets share one backend.
let activeCaptures = 0;

function setSuspended(suspended: boolean): void {
  activeCaptures += suspended ? 1 : -1;
  const shouldSuspend = activeCaptures > 0;
  if (shouldSuspend === suspended) {
    void ipc.setHotkeysSuspended(suspended).catch((error) => {
      console.error("[DofusTabs] failed to toggle hotkey suspension", error);
    });
  }
}

/** Only exported for tests, to reset shared module state between cases. */
export function __resetActiveCapturesForTests(): void {
  activeCaptures = 0;
}

export function createHotkeyCapture(options: HotkeyCaptureOptions): HotkeyCapture {
  let current = options.initial;

  const el = document.createElement("button");
  el.type = "button";
  el.className = "hotkey-capture";
  if (options.compact) el.classList.add("compact");
  el.title = "Clic para asignar · Retroceso borra · Esc cancela";
  el.setAttribute("aria-label", options.ariaLabel ?? "Asignar atajo de teclado");

  const idle = () => {
    const text = formatHotkey(current);
    el.textContent = text || (options.placeholder ?? "Sin atajo");
    el.classList.toggle("empty", !text);
  };

  // Watches for the element being removed from the document while it is
  // capturing (e.g. a parent rebuild that forgot to call stop() first) and
  // force-stops it so the shared suspension refcount never leaks.
  let disconnectWatcher: MutationObserver | null = null;

  const stopWatchingDisconnect = () => {
    disconnectWatcher?.disconnect();
    disconnectWatcher = null;
  };

  // Every exit path (Escape, Backspace, a captured combination, blur, or a
  // forced cleanup) goes through here, so the suspension is always lifted
  // exactly once.
  const stop = () => {
    if (!el.classList.contains("capturing")) return;
    el.classList.remove("capturing");
    el.removeEventListener("keydown", onKeyDown);
    stopWatchingDisconnect();
    setSuspended(false);
    idle();
    // The element may already be detached from the document (e.g. the
    // settings panel was torn down mid-capture); blur() is a no-op then.
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
    setSuspended(true);
    el.focus();

    if (el.isConnected && el.parentNode) {
      disconnectWatcher = new MutationObserver(() => {
        if (!el.isConnected) stop();
      });
      disconnectWatcher.observe(document.body, { childList: true, subtree: true });
    }
  });

  el.addEventListener("blur", () => {
    if (el.classList.contains("capturing")) stop();
  });

  idle();

  return {
    element: el,
    setValue(hotkey) {
      // Never clobber a combination the user is in the middle of pressing.
      if (el.classList.contains("capturing")) return;
      current = hotkey;
      idle();
    },
  };
}
