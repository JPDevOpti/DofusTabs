import type { HotkeyView, InstanceView } from "../ipc";
import { createHotkeyCapture } from "./hotkey-capture";

export interface AccountItemHandlers {
  onActivate(pid: number): void;
  onContextMenu(pid: number, x: number, y: number): void;
  onDragStart(pid: number): void;
  onDragEnd(): void;
  onDropOn(pid: number): void;
  onHotkeyChange(pid: number, hotkey: HotkeyView | null): void;
}

export function createAccountItem(
  instance: InstanceView,
  showNames: boolean,
  iconUrl: string | null,
  handlers: AccountItemHandlers,
): HTMLElement {
  const item = document.createElement("div");
  item.className = "account-item";
  item.classList.toggle("active", instance.isActive);
  item.classList.toggle("disabled", !instance.enabled);
  item.draggable = true;
  if (!showNames) {
    item.title = instance.className
      ? `${instance.characterName} · ${instance.className}`
      : instance.characterName;
  }

  const icon = document.createElement("div");
  icon.className = "account-icon";
  if (iconUrl) {
    const img = document.createElement("img");
    img.src = iconUrl;
    img.alt = instance.className ?? "";
    img.draggable = false;
    icon.appendChild(img);
  } else {
    icon.classList.add("fallback");
    icon.textContent = (instance.characterName.trim()[0] ?? "?").toUpperCase();
  }
  item.appendChild(icon);

  const body = document.createElement("div");
  body.className = "account-body";
  if (showNames) {
    const name = document.createElement("span");
    name.className = "account-name";
    name.textContent = instance.characterName || "(sin nombre)";
    body.appendChild(name);

    if (instance.className) {
      const cls = document.createElement("span");
      cls.className = "account-class";
      cls.textContent = instance.className;
      body.appendChild(cls);
    }
  }
  item.appendChild(body);

  if (instance.isPrimary) {
    const star = document.createElement("span");
    star.className = "primary-star";
    star.textContent = "★";
    star.title = "Personaje principal";
    item.appendChild(star);
  }

  const badge = createHotkeyCapture({
    initial: instance.hotkey,
    placeholder: "Atajo",
    compact: true,
    onChange: (hotkey) => handlers.onHotkeyChange(instance.pid, hotkey),
  });
  badge.classList.add("account-hotkey");
  item.appendChild(badge);

  item.addEventListener("click", (event) => {
    if ((event.target as HTMLElement).closest(".hotkey-capture")) return;
    if (!instance.enabled) return;
    handlers.onActivate(instance.pid);
  });

  item.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    handlers.onContextMenu(instance.pid, event.clientX, event.clientY);
  });

  item.addEventListener("dragstart", (event) => {
    event.dataTransfer?.setData("text/plain", String(instance.pid));
    item.classList.add("dragging");
    handlers.onDragStart(instance.pid);
  });
  item.addEventListener("dragend", () => {
    item.classList.remove("dragging");
    handlers.onDragEnd();
  });
  item.addEventListener("dragover", (event) => {
    event.preventDefault();
    item.classList.add("drop-target");
  });
  item.addEventListener("dragleave", () => item.classList.remove("drop-target"));
  item.addEventListener("drop", (event) => {
    event.preventDefault();
    item.classList.remove("drop-target");
    handlers.onDropOn(instance.pid);
  });

  return item;
}
