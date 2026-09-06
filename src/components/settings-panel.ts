import type { GlobalHotkeySlot, HotkeyView, SettingsView } from "../ipc";
import { createHotkeyCapture } from "./hotkey-capture";

export interface SettingsPanelHandlers {
  onGlobalHotkeyChange(slot: GlobalHotkeySlot, hotkey: HotkeyView | null): void;
}

export interface SettingsPanel {
  element: HTMLElement;
  update(settings: SettingsView): void;
  toggle(): void;
}

export function createSettingsPanel(handlers: SettingsPanelHandlers): SettingsPanel {
  const element = document.createElement("section");
  element.className = "settings-panel collapsed";

  const title = document.createElement("h2");
  title.textContent = "Atajos globales";
  element.appendChild(title);

  const rows = document.createElement("div");
  rows.className = "settings-rows";
  element.appendChild(rows);

  const build = (settings: SettingsView) => {
    rows.replaceChildren();

    const definitions: Array<[GlobalHotkeySlot, string, HotkeyView]> = [
      ["next", "Siguiente cuenta", settings.nextHotkey],
      ["previous", "Cuenta anterior", settings.previousHotkey],
      ["primary", "Cuenta principal", settings.primaryHotkey],
    ];

    for (const [slot, label, hotkey] of definitions) {
      const row = document.createElement("div");
      row.className = "settings-row";

      const caption = document.createElement("span");
      caption.textContent = label;
      row.appendChild(caption);

      row.appendChild(
        createHotkeyCapture({
          initial: hotkey.key ? hotkey : null,
          onChange: (value) => handlers.onGlobalHotkeyChange(slot, value),
        }),
      );

      rows.appendChild(row);
    }
  };

  return {
    element,
    update: build,
    toggle: () => element.classList.toggle("collapsed"),
  };
}
