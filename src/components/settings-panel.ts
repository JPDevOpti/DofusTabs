import type { GlobalHotkeySlot, HotkeyView, SettingsView } from "../ipc";
import { createHotkeyCapture, type HotkeyCapture } from "./hotkey-capture";

export interface SettingsPanelHandlers {
  onLaunchGame(): void;
  onRefresh(): void;
  onToggleShowNames(show: boolean): void;
  onGlobalHotkeyChange(slot: GlobalHotkeySlot, hotkey: HotkeyView | null): void;
  onToggleManageGameWindows(manage: boolean): void;
  onArrangeWindows(): void;
  onLauncherPathChange(path: string | null): void;
  onClose(): void;
}

export interface SettingsPanel {
  element: HTMLElement;
  update(settings: SettingsView | null, showNames: boolean): void;
  open(): void;
  close(): void;
  toggle(): void;
  isOpen(): boolean;
}

const HOTKEY_DEFINITIONS: Array<[GlobalHotkeySlot, string, (s: SettingsView) => HotkeyView]> = [
  ["next", "Siguiente cuenta", (s) => s.nextHotkey],
  ["previous", "Cuenta anterior", (s) => s.previousHotkey],
  ["primary", "Cuenta principal", (s) => s.primaryHotkey],
];

export function createSettingsPanel(handlers: SettingsPanelHandlers): SettingsPanel {
  const element = document.createElement("section");
  element.className = "settings-panel hidden";

  const header = document.createElement("div");
  header.className = "settings-header";
  const heading = document.createElement("h2");
  heading.className = "settings-heading";
  heading.textContent = "Ajustes";
  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.className = "icon-btn settings-close";
  closeButton.title = "Cerrar";
  closeButton.setAttribute("aria-label", "Cerrar ajustes");
  closeButton.textContent = "✕";
  closeButton.addEventListener("click", () => handlers.onClose());
  header.append(heading, closeButton);
  element.appendChild(header);

  const section = (heading: string, ...children: HTMLElement[]): HTMLElement => {
    const wrapper = document.createElement("div");
    wrapper.className = "settings-section";
    const title = document.createElement("h2");
    title.textContent = heading;
    wrapper.append(title, ...children);
    return wrapper;
  };

  const actionButton = (icon: string, label: string, title: string, onClick: () => void) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "settings-btn";
    button.title = title;
    button.setAttribute("aria-label", title);
    const glyph = document.createElement("span");
    glyph.className = "btn-icon";
    glyph.setAttribute("aria-hidden", "true");
    glyph.textContent = icon;
    button.append(glyph, document.createTextNode(label));
    button.addEventListener("click", onClick);
    return button;
  };

  const actionsRow = document.createElement("div");
  actionsRow.className = "settings-actions";
  actionsRow.append(
    actionButton("▶", "Abrir launcher", "Abre el Ankama Launcher usando la GPU dedicada", () =>
      handlers.onLaunchGame(),
    ),
    actionButton("↻", "Refrescar", "Vuelve a buscar clientes de Dofus abiertos", () =>
      handlers.onRefresh(),
    ),
  );

  const showNamesLabel = document.createElement("label");
  showNamesLabel.className = "settings-check";
  showNamesLabel.htmlFor = "settings-show-names";
  const showNamesInput = document.createElement("input");
  showNamesInput.type = "checkbox";
  showNamesInput.id = "settings-show-names";
  showNamesInput.addEventListener("change", () =>
    handlers.onToggleShowNames(showNamesInput.checked),
  );
  showNamesLabel.append(
    showNamesInput,
    document.createTextNode("Mostrar nombres bajo cada cuenta"),
  );

  element.appendChild(section("Cuentas", actionsRow, showNamesLabel));

  const rows = document.createElement("div");
  rows.className = "settings-rows";
  element.appendChild(section("Atajos globales", rows));

  // Built once so an in-progress capture is never torn down mid-press; see
  // update() below, which only pushes new values into the existing widgets.
  const hotkeyCaptures = new Map<GlobalHotkeySlot, HotkeyCapture>();
  for (const [slot, label] of HOTKEY_DEFINITIONS) {
    const row = document.createElement("div");
    row.className = "settings-row";

    const caption = document.createElement("span");
    caption.textContent = label;
    row.appendChild(caption);

    const capture = createHotkeyCapture({
      initial: null,
      ariaLabel: `Asignar atajo: ${label}`,
      onChange: (value) => handlers.onGlobalHotkeyChange(slot, value),
    });
    hotkeyCaptures.set(slot, capture);
    row.appendChild(capture.element);

    rows.appendChild(row);
  }

  const manageLabel = document.createElement("label");
  manageLabel.className = "settings-check";
  manageLabel.htmlFor = "settings-manage-windows";
  const manageInput = document.createElement("input");
  manageInput.type = "checkbox";
  manageInput.id = "settings-manage-windows";
  manageInput.addEventListener("change", () =>
    handlers.onToggleManageGameWindows(manageInput.checked),
  );
  manageLabel.append(
    manageInput,
    document.createTextNode("Gestionar ventanas del juego (flotantes, apiladas a la derecha)"),
  );

  const arrangeButton = actionButton(
    "⊞",
    "Reorganizar ventanas",
    "Vuelve a apilar todos los clientes",
    () => handlers.onArrangeWindows(),
  );

  element.appendChild(section("Ventanas", manageLabel, arrangeButton));

  const launcherInput = document.createElement("input");
  launcherInput.type = "text";
  launcherInput.className = "settings-input";
  launcherInput.placeholder = "~/Applications/Dofus.AppImage";
  launcherInput.spellcheck = false;
  launcherInput.setAttribute("aria-label", "Ruta del Ankama Launcher");
  const launcherHint = document.createElement("p");
  launcherHint.className = "settings-hint";
  launcherHint.textContent = "Ruta del Ankama Launcher. Vacío usa la ruta por defecto.";
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
  element.appendChild(section("Launcher", launcherInput, launcherHint));

  const build = (settings: SettingsView | null, showNames: boolean) => {
    showNamesInput.checked = showNames;
    if (!settings) return;

    for (const [slot, , getValue] of HOTKEY_DEFINITIONS) {
      const hotkey = getValue(settings);
      hotkeyCaptures.get(slot)?.setValue(hotkey.key ? hotkey : null);
    }

    manageInput.checked = settings.manageGameWindows;
    arrangeButton.disabled = !settings.manageGameWindows;
    // Do not clobber what the user is typing right now.
    if (document.activeElement !== launcherInput) launcherInput.value = settings.launcherPath;
  };

  return {
    element,
    update: build,
    open: () => element.classList.remove("hidden"),
    close: () => element.classList.add("hidden"),
    toggle: () => element.classList.toggle("hidden"),
    isOpen: () => !element.classList.contains("hidden"),
  };
}
