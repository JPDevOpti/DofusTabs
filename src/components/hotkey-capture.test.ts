import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { __resetActiveCapturesForTests, createHotkeyCapture, formatHotkey } from "./hotkey-capture";

describe("formatHotkey", () => {
  it("returns an empty string for no hotkey", () => {
    expect(formatHotkey(null)).toBe("");
    expect(formatHotkey({ ctrl: false, alt: false, shift: false, superKey: false, key: "" })).toBe(
      "",
    );
  });

  it("joins modifiers and key in order", () => {
    expect(formatHotkey({ ctrl: true, alt: true, shift: false, superKey: false, key: "a" })).toBe(
      "Ctrl+Alt+A",
    );
  });

  it("keeps multi-character key names as-is", () => {
    expect(formatHotkey({ ctrl: false, alt: false, shift: false, superKey: true, key: "F5" })).toBe(
      "Super+F5",
    );
  });
});

describe("createHotkeyCapture suspension", () => {
  beforeEach(() => {
    invoke.mockClear();
    __resetActiveCapturesForTests();
  });

  it("suspends on capture start and resumes on stop", () => {
    const capture = createHotkeyCapture({ initial: null, onChange: () => {} });
    document.body.appendChild(capture.element);

    capture.element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(invoke).toHaveBeenCalledWith("set_hotkeys_suspended", { suspended: true });

    capture.element.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(invoke).toHaveBeenLastCalledWith("set_hotkeys_suspended", { suspended: false });
  });

  it("does not leak the suspension refcount when a capturing widget is detached and rebuilt", async () => {
    const capture = createHotkeyCapture({ initial: null, onChange: () => {} });
    document.body.appendChild(capture.element);

    // Start a capture, then detach without an explicit stop() — simulating
    // a parent rebuild mid-capture.
    capture.element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    capture.element.remove();

    // The disconnect watcher fires via a MutationObserver microtask.
    await new Promise<void>((resolve) => queueMicrotask(resolve));

    // A fresh widget must still be able to suspend/resume correctly; if the
    // refcount leaked, the second suspend call above would have kept the
    // shared counter above zero forever.
    const second = createHotkeyCapture({ initial: null, onChange: () => {} });
    document.body.appendChild(second.element);
    second.element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    second.element.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

    // The final resume call should bring suspension back to false.
    expect(invoke).toHaveBeenLastCalledWith("set_hotkeys_suspended", { suspended: false });
  });

  it("setValue ignores updates while capturing", () => {
    const onChange = vi.fn();
    const capture = createHotkeyCapture({ initial: null, onChange });
    document.body.appendChild(capture.element);

    capture.element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    capture.setValue({ ctrl: true, alt: false, shift: false, superKey: false, key: "X" });
    expect(capture.element.textContent).not.toBe("Ctrl+X");

    capture.element.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    capture.setValue({ ctrl: true, alt: false, shift: false, superKey: false, key: "X" });
    expect(capture.element.textContent).toBe("Ctrl+X");
  });
});
