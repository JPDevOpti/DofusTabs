import type { GlobalHotkeySlot, HotkeyView, InstanceView, SettingsView } from "../ipc";
import { iconUrlFor } from "../icons";
import { createAccountItem, type AccountItemHandlers } from "./account-item";
import { createSettingsPanel } from "./settings-panel";

export interface SidebarState {
  instances: InstanceView[];
  showNames: boolean;
  settings: SettingsView | null;
}

export interface SidebarHandlers {
  onRefresh(): void;
  onToggleShowNames(show: boolean): void;
  onActivate(pid: number): void;
  onReorder(pids: number[]): void;
  onMoveUp(pid: number): void;
  onMoveDown(pid: number): void;
  onSetEnabled(pid: number, enabled: boolean): void;
  onSetPrimary(pid: number): void;
  onInstanceHotkeyChange(pid: number, hotkey: HotkeyView | null): void;
  onGlobalHotkeyChange(slot: GlobalHotkeySlot, hotkey: HotkeyView | null): void;
}

export interface Sidebar {
  update(state: SidebarState): void;
}

export function createSidebar(root: HTMLElement, handlers: SidebarHandlers): Sidebar {
  let currentState: SidebarState = { instances: [], showNames: false, settings: null };
  let draggedPid: number | null = null;

  root.replaceChildren();
  root.className = "sidebar";

  // Top bar
  const topbar = document.createElement("header");
  topbar.className = "topbar";

  const brand = document.createElement("div");
  brand.className = "brand";
  const brandTitle = document.createElement("span");
  brandTitle.className = "brand-title";
  brandTitle.textContent = "DofusTabs";
  const counter = document.createElement("span");
  counter.className = "counter";
  counter.title = "Cuentas habilitadas / detectadas";
  brand.append(brandTitle, counter);

  const actions = document.createElement("div");
  actions.className = "topbar-actions";
  const refreshButton = document.createElement("button");
  refreshButton.type = "button";
  refreshButton.className = "icon-btn";
  refreshButton.title = "Refrescar";
  refreshButton.textContent = "↻";
  refreshButton.addEventListener("click", () => handlers.onRefresh());
  const settingsButton = document.createElement("button");
  settingsButton.type = "button";
  settingsButton.className = "icon-btn";
  settingsButton.title = "Atajos globales";
  settingsButton.textContent = "⚙";
  actions.append(refreshButton, settingsButton);

  topbar.append(brand, actions);

  // Collapsible global hotkeys panel
  const settingsPanel = createSettingsPanel({
    onGlobalHotkeyChange: handlers.onGlobalHotkeyChange,
  });
  settingsButton.addEventListener("click", () => settingsPanel.toggle());

  // Show-names toggle
  const showNamesLabel = document.createElement("label");
  showNamesLabel.className = "show-names";
  const showNamesInput = document.createElement("input");
  showNamesInput.type = "checkbox";
  showNamesInput.addEventListener("change", () => handlers.onToggleShowNames(showNamesInput.checked));
  showNamesLabel.append(showNamesInput, document.createTextNode("Mostrar nombres"));

  // Accounts list + empty state
  const list = document.createElement("main");
  list.className = "accounts";
  const empty = document.createElement("div");
  empty.className = "empty-state hidden";
  empty.textContent = "Abre clientes de Dofus para verlos aquí";

  root.append(topbar, settingsPanel.element, showNamesLabel, list);

  // Context menu (attached to body so it can overflow the list)
  const menu = document.createElement("div");
  menu.className = "context-menu hidden";
  document.body.appendChild(menu);
  const closeMenu = () => menu.classList.add("hidden");
  document.addEventListener("click", closeMenu);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeMenu();
  });
  window.addEventListener("blur", closeMenu);

  const menuItem = (label: string, action: () => void, enabled = true): HTMLElement => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "context-menu-item";
    button.textContent = label;
    button.disabled = !enabled;
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      closeMenu();
      action();
    });
    return button;
  };

  const openMenu = (pid: number, x: number, y: number) => {
    const instance = currentState.instances.find((i) => i.pid === pid);
    if (!instance) return;

    // Moves stay inside the account's own enabled/disabled group.
    const group = orderedInstances().filter((i) => i.enabled === instance.enabled);
    const position = group.findIndex((i) => i.pid === pid);
    const canMoveUp = position > 0;
    const canMoveDown = position >= 0 && position < group.length - 1;

    menu.replaceChildren(
      menuItem("Subir", () => handlers.onMoveUp(pid), canMoveUp),
      menuItem("Bajar", () => handlers.onMoveDown(pid), canMoveDown),
      menuItem(instance.enabled ? "Deshabilitar cuenta" : "Habilitar cuenta", () =>
        handlers.onSetEnabled(pid, !instance.enabled),
      ),
      menuItem(
        instance.isPrimary ? "Ya es el personaje principal" : "Volver principal a este",
        () => handlers.onSetPrimary(pid),
        !instance.isPrimary,
      ),
    );

    menu.classList.remove("hidden");
    const rect = menu.getBoundingClientRect();
    menu.style.left = `${Math.max(0, Math.min(x, window.innerWidth - rect.width - 8))}px`;
    menu.style.top = `${Math.max(0, Math.min(y, window.innerHeight - rect.height - 8))}px`;
  };

  // Same order the backend uses for its views: enabled first, then by
  // display order within each group.
  const orderedInstances = () =>
    [...currentState.instances].sort(
      (a, b) => Number(!a.enabled) - Number(!b.enabled) || a.displayOrder - b.displayOrder,
    );

  const handleDrop = (targetPid: number) => {
    if (draggedPid === null || draggedPid === targetPid) return;
    const pids = orderedInstances().map((i) => i.pid);
    const from = pids.indexOf(draggedPid);
    const to = pids.indexOf(targetPid);
    if (from < 0 || to < 0) return;
    pids.splice(from, 1);
    pids.splice(to, 0, draggedPid);
    handlers.onReorder(pids);
  };

  const itemHandlers: AccountItemHandlers = {
    onActivate: handlers.onActivate,
    onContextMenu: openMenu,
    onDragStart: (pid) => {
      draggedPid = pid;
    },
    onDragEnd: () => {
      draggedPid = null;
    },
    onDropOn: handleDrop,
    onHotkeyChange: handlers.onInstanceHotkeyChange,
  };

  const render = () => {
    const ordered = orderedInstances();
    const enabledCount = ordered.filter((i) => i.enabled).length;
    counter.textContent = `${enabledCount}/${ordered.length}`;
    showNamesInput.checked = currentState.showNames;

    list.replaceChildren();
    if (ordered.length === 0) {
      empty.classList.remove("hidden");
      list.appendChild(empty);
    } else {
      empty.classList.add("hidden");
      for (const instance of ordered) {
        list.appendChild(
          createAccountItem(
            instance,
            currentState.showNames,
            iconUrlFor(instance.iconFile),
            itemHandlers,
          ),
        );
      }
    }

    if (currentState.settings) settingsPanel.update(currentState.settings);
  };

  return {
    update(state) {
      currentState = state;
      render();
    },
  };
}
