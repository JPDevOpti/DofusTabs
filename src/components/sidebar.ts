import { iconUrlFor } from "../class-icons";
import type { GlobalHotkeySlot, HotkeyView, InstanceView, SettingsView } from "../ipc";
import { type AccountItem, type AccountItemHandlers, createAccountItem } from "./account-item";
import { createHotkeyCapture } from "./hotkey-capture";
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
  onToggleManageGameWindows(manage: boolean): void;
  onArrangeWindows(): void;
  onLauncherPathChange(path: string | null): void;
  onLaunchGame(): void;
  onSetSidebarExtension(extra: number): void;
}

export interface Sidebar {
  update(state: SidebarState): void;
}

function sidebarWidthPx(): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue("--sidebar-width");
  const parsed = Number.parseFloat(raw);
  return Number.isFinite(parsed) ? parsed : 64;
}

// Measures how far past the sidebar column an overlay actually extends,
// instead of hardcoding its width (which used to drift from styles.css).
function measureExtension(el: HTMLElement): number {
  const rect = el.getBoundingClientRect();
  if (rect.width === 0) return 0;
  return Math.max(0, Math.ceil(rect.right - sidebarWidthPx() + 8));
}

export function createSidebar(root: HTMLElement, handlers: SidebarHandlers): Sidebar {
  let currentState: SidebarState = { instances: [], showNames: false, settings: null };
  let draggedPid: number | null = null;
  let openPopoverPid: number | null = null;

  root.replaceChildren();
  root.className = "sidebar";

  // Accounts list
  const list = document.createElement("div");
  list.className = "accounts";

  // Settings toggle, pinned at the bottom of the column
  const settingsButton = document.createElement("button");
  settingsButton.type = "button";
  settingsButton.className = "icon-btn settings-toggle";
  settingsButton.title = "Ajustes";
  settingsButton.setAttribute("aria-label", "Abrir ajustes");
  settingsButton.textContent = "⚙︎";

  // Settings overlay panel, anchored to the right of the column
  const settingsPanel = createSettingsPanel({
    onLaunchGame: handlers.onLaunchGame,
    onRefresh: handlers.onRefresh,
    onToggleShowNames: handlers.onToggleShowNames,
    onGlobalHotkeyChange: handlers.onGlobalHotkeyChange,
    onToggleManageGameWindows: handlers.onToggleManageGameWindows,
    onArrangeWindows: handlers.onArrangeWindows,
    onLauncherPathChange: handlers.onLauncherPathChange,
    onClose: () => {
      settingsPanel.close();
      syncSidebarExtension();
    },
  });

  // Widens the own window while the settings panel or the account popover
  // extends past the sidebar column, and shrinks it back once both are
  // closed. Measured from the live elements instead of duplicating the CSS
  // widths here.
  const syncSidebarExtension = () => {
    const extra = Math.max(
      settingsPanel.isOpen() ? measureExtension(settingsPanel.element) : 0,
      popover.classList.contains("hidden") ? 0 : measureExtension(popover),
    );
    handlers.onSetSidebarExtension(extra);
  };

  settingsButton.addEventListener("click", () => {
    closePopover();
    settingsPanel.toggle();
    syncSidebarExtension();
  });

  const divider = document.createElement("div");
  divider.className = "sidebar-divider";

  root.append(list, divider, settingsButton, settingsPanel.element);

  // Per-account controls popover, anchored to the right of the column at the
  // tile's vertical offset. Attached to body so it can overflow the column.
  const popover = document.createElement("div");
  popover.className = "account-popover hidden";
  document.body.appendChild(popover);
  const closePopover = () => {
    popover.classList.add("hidden");
    openPopoverPid = null;
    syncSidebarExtension();
  };

  document.addEventListener("click", (event) => {
    if (!popover.classList.contains("hidden") && !popover.contains(event.target as Node)) {
      closePopover();
    }
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closePopover();
      settingsPanel.close();
      syncSidebarExtension();
    }
  });
  window.addEventListener("blur", closePopover);

  const popoverAction = (label: string, action: () => void, enabled = true): HTMLElement => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "popover-item";
    button.textContent = label;
    button.disabled = !enabled;
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      closePopover();
      action();
    });
    return button;
  };

  const renderPopoverContents = (instance: InstanceView, ordered: InstanceView[]) => {
    // Moves stay inside the account's own enabled/disabled group.
    const group = ordered.filter((i) => i.enabled === instance.enabled);
    const position = group.findIndex((i) => i.pid === instance.pid);
    const canMoveUp = position > 0;
    const canMoveDown = position >= 0 && position < group.length - 1;

    const hotkeyRow = document.createElement("div");
    hotkeyRow.className = "popover-row";
    const hotkeyLabel = document.createElement("span");
    hotkeyLabel.textContent = "Atajo";
    hotkeyRow.appendChild(hotkeyLabel);
    const popoverHotkeyCapture = createHotkeyCapture({
      initial: instance.hotkey,
      placeholder: "Sin atajo",
      compact: true,
      ariaLabel: "Asignar atajo de esta cuenta",
      onChange: (hotkey) => handlers.onInstanceHotkeyChange(instance.pid, hotkey),
    });
    hotkeyRow.appendChild(popoverHotkeyCapture.element);

    popover.replaceChildren(
      popoverAction("Subir", () => handlers.onMoveUp(instance.pid), canMoveUp),
      popoverAction("Bajar", () => handlers.onMoveDown(instance.pid), canMoveDown),
      popoverAction(instance.enabled ? "Deshabilitar cuenta" : "Habilitar cuenta", () =>
        handlers.onSetEnabled(instance.pid, !instance.enabled),
      ),
      popoverAction(
        instance.isPrimary ? "Ya es el personaje principal" : "Volver principal a este",
        () => handlers.onSetPrimary(instance.pid),
        !instance.isPrimary,
      ),
      hotkeyRow,
    );
  };

  const openPopover = (pid: number, anchorTop: number) => {
    const instance = currentState.instances.find((i) => i.pid === pid);
    if (!instance) return;
    settingsPanel.close();
    openPopoverPid = pid;

    renderPopoverContents(instance, orderedInstances());

    popover.classList.remove("hidden");
    const maxTop = window.innerHeight - popover.offsetHeight - 8;
    popover.style.top = `${Math.max(8, Math.min(anchorTop, maxTop))}px`;
    syncSidebarExtension();
  };

  // Same order the backend uses for its views: enabled first, then by
  // display order within each group.
  const orderedInstances = () =>
    [...currentState.instances].sort(
      (a, b) => Number(!a.enabled) - Number(!b.enabled) || a.displayOrder - b.displayOrder,
    );

  const handleDrop = (targetPid: number, ordered: InstanceView[]) => {
    if (draggedPid === null || draggedPid === targetPid) return;
    const pids = ordered.map((i) => i.pid);
    const from = pids.indexOf(draggedPid);
    const to = pids.indexOf(targetPid);
    if (from < 0 || to < 0) return;
    pids.splice(from, 1);
    pids.splice(to, 0, draggedPid);
    handlers.onReorder(pids);
  };

  const itemHandlers: AccountItemHandlers = {
    onActivate: handlers.onActivate,
    onOpenPopover: openPopover,
    onDragStart: (pid) => {
      draggedPid = pid;
    },
    onDragEnd: () => {
      draggedPid = null;
    },
    onDropOn: (pid) => handleDrop(pid, orderedInstances()),
  };

  const items = new Map<number, AccountItem>();

  const render = () => {
    const ordered = orderedInstances();
    const seen = new Set<number>();

    let previousElement: HTMLElement | null = null;
    for (const instance of ordered) {
      seen.add(instance.pid);
      let item = items.get(instance.pid);
      if (!item) {
        item = createAccountItem(
          instance,
          currentState.showNames,
          iconUrlFor(instance.iconFile),
          itemHandlers,
        );
        items.set(instance.pid, item);
      } else {
        item.update(instance, currentState.showNames, iconUrlFor(instance.iconFile));
      }

      // Reorder only when the element isn't already right after the
      // previous one, so unaffected nodes (and their transitions) are left
      // alone.
      const expectedNext: Element | null = previousElement
        ? previousElement.nextElementSibling
        : list.firstElementChild;
      if (expectedNext !== item.element) {
        const insertBeforeNode = previousElement ? previousElement.nextSibling : list.firstChild;
        list.insertBefore(item.element, insertBeforeNode);
      }
      previousElement = item.element;
    }

    for (const [pid, item] of items) {
      if (!seen.has(pid)) {
        item.element.remove();
        items.delete(pid);
      }
    }

    settingsPanel.update(currentState.settings, currentState.showNames);

    // Keep the open popover in sync (canMoveUp/canMoveDown, hotkey, enabled
    // state) instead of letting it go stale until closed.
    if (openPopoverPid !== null) {
      const instance = currentState.instances.find((i) => i.pid === openPopoverPid);
      if (!instance) {
        closePopover();
      } else {
        renderPopoverContents(instance, ordered);
      }
    }
  };

  return {
    update(state) {
      currentState = state;
      render();
    },
  };
}
