import type { GlobalHotkeySlot, HotkeyView, SettingsView } from "../ipc";
import { createHotkeyCapture } from "./hotkey-capture";

export interface SettingsPanelHandlers {
  onGlobalHotkeyChange(slot: GlobalHotkeySlot, hotkey: HotkeyView | null): void;
  onToggleManageGameWindows(manage: boolean): void;
  onArrangeWindows(): void;
  onLauncherPathChange(path: string | null): void;
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

  const windowsTitle = document.createElement("h2");
  windowsTitle.textContent = "Ventanas";
  element.appendChild(windowsTitle);

  const manageLabel = document.createElement("label");
  manageLabel.className = "settings-check";
  const manageInput = document.createElement("input");
  manageInput.type = "checkbox";
  manageInput.addEventListener("change", () =>
    handlers.onToggleManageGameWindows(manageInput.checked),
  );
  manageLabel.append(
    manageInput,
    document.createTextNode("Gestionar ventanas del juego (flotantes, apiladas en mi workspace)"),
  );
  element.appendChild(manageLabel);

  const arrangeButton = document.createElement("button");
  arrangeButton.type = "button";
  arrangeButton.className = "settings-btn";
  arrangeButton.textContent = "Reorganizar ventanas";
  arrangeButton.addEventListener("click", () => handlers.onArrangeWindows());
  element.appendChild(arrangeButton);

  const launcherTitle = document.createElement("h2");
  launcherTitle.textContent = "Launcher";
  element.appendChild(launcherTitle);

  const launcherInput = document.createElement("input");
  launcherInput.type = "text";
  launcherInput.className = "settings-input";
  launcherInput.placeholder = "Ruta del Ankama Launcher (AppImage)";
  launcherInput.title = "Vacío = ~/Applications/Dofus.AppImage";
  launcherInput.spellcheck = false;
  // Commit on blur/Enter, not on every keystroke, so a half-typed path is
  // never persisted.
  const commitLauncherPath = () => {
    const value = launcherInput.value.trim();
    handlers.onLauncherPathChange(value === "" ? null : value);
  };
  launcherInput.addEventListener("change", commitLauncherPath);
  launcherInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") launcherInput.blur();
  });
  element.appendChild(launcherInput);

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

    manageInput.checked = settings.manageGameWindows;
    arrangeButton.disabled = !settings.manageGameWindows;
    // Do not clobber what the user is typing right now.
    if (document.activeElement !== launcherInput) launcherInput.value = settings.launcherPath;
  };

  return {
    element,
    update: build,
    toggle: () => element.classList.toggle("collapsed"),
  };
}
