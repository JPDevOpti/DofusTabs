import type { GpuKind, InstanceView } from "../ipc";

export interface AccountItemHandlers {
  onActivate(pid: number): void;
  onOpenPopover(pid: number, anchorTop: number): void;
  onDragStart(pid: number): void;
  onDragEnd(): void;
  onDropOn(pid: number): void;
}

export interface AccountItem {
  element: HTMLElement;
  update(instance: InstanceView, showNames: boolean, iconUrl: string | null): void;
}

const GPU_TITLES: Record<Exclude<GpuKind, "unknown">, string> = {
  dedicated: "Renderiza en la tarjeta gráfica dedicada",
  integrated: "Renderiza en la gráfica integrada",
};

// A coloured dot on the tile corner; the meaning lives in the tooltip and
// the accessible name.
function createGpuBadge(gpu: GpuKind): HTMLElement | null {
  if (gpu === "unknown") return null;
  const badge = document.createElement("span");
  badge.className = `gpu-badge ${gpu}`;
  badge.title = GPU_TITLES[gpu];
  badge.setAttribute("role", "img");
  badge.setAttribute("aria-label", GPU_TITLES[gpu]);
  return badge;
}

export function createAccountItem(
  instance: InstanceView,
  showNames: boolean,
  iconUrl: string | null,
  handlers: AccountItemHandlers,
): AccountItem {
  let currentPid = instance.pid;

  const item = document.createElement("div");
  item.className = "account-item";
  item.draggable = true;

  const tile = document.createElement("div");
  tile.className = "account-tile";

  const icon = document.createElement("div");
  icon.className = "account-icon";
  tile.appendChild(icon);

  let gpuBadge: HTMLElement | null = null;
  let star: HTMLElement | null = null;

  const moreButton = document.createElement("button");
  moreButton.type = "button";
  moreButton.className = "account-more";
  moreButton.title = "Más opciones";
  moreButton.setAttribute("aria-label", "Más opciones de la cuenta");
  moreButton.textContent = "⋯";
  moreButton.addEventListener("click", (event) => {
    event.stopPropagation();
    handlers.onOpenPopover(currentPid, tile.getBoundingClientRect().top);
  });
  tile.appendChild(moreButton);

  item.appendChild(tile);

  let name: HTMLElement | null = null;

  item.addEventListener("click", (event) => {
    if ((event.target as HTMLElement).closest(".account-more")) return;
    if (!item.classList.contains("disabled")) handlers.onActivate(currentPid);
  });

  item.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    handlers.onOpenPopover(currentPid, tile.getBoundingClientRect().top);
  });

  item.addEventListener("dragstart", (event) => {
    event.dataTransfer?.setData("text/plain", String(currentPid));
    item.classList.add("dragging");
    handlers.onDragStart(currentPid);
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
    handlers.onDropOn(currentPid);
  });

  const update = (
    nextInstance: InstanceView,
    nextShowNames: boolean,
    nextIconUrl: string | null,
  ) => {
    currentPid = nextInstance.pid;

    item.classList.toggle("active", nextInstance.isActive);
    item.classList.toggle("disabled", !nextInstance.enabled);
    item.classList.toggle("has-name", nextShowNames);
    item.title = nextInstance.className
      ? `${nextInstance.characterName} · ${nextInstance.className}`
      : nextInstance.characterName;

    icon.replaceChildren();
    if (nextIconUrl) {
      icon.classList.remove("fallback");
      const img = document.createElement("img");
      img.src = nextIconUrl;
      img.alt = nextInstance.className ?? "";
      img.draggable = false;
      icon.appendChild(img);
    } else {
      icon.classList.add("fallback");
      icon.textContent = (nextInstance.characterName.trim()[0] ?? "?").toUpperCase();
    }

    if (nextInstance.gpu !== "unknown") {
      if (!gpuBadge) {
        gpuBadge = createGpuBadge(nextInstance.gpu);
        if (gpuBadge) tile.insertBefore(gpuBadge, moreButton);
      } else {
        gpuBadge.className = `gpu-badge ${nextInstance.gpu}`;
        gpuBadge.title = GPU_TITLES[nextInstance.gpu];
        gpuBadge.setAttribute("aria-label", GPU_TITLES[nextInstance.gpu]);
      }
    } else if (gpuBadge) {
      gpuBadge.remove();
      gpuBadge = null;
    }

    if (nextInstance.isPrimary) {
      if (!star) {
        star = document.createElement("span");
        star.className = "primary-star";
        star.textContent = "★";
        star.title = "Personaje principal";
        tile.insertBefore(star, moreButton);
      }
    } else if (star) {
      star.remove();
      star = null;
    }

    if (nextShowNames) {
      if (!name) {
        name = document.createElement("span");
        name.className = "account-name";
        item.appendChild(name);
      }
      name.textContent = nextInstance.characterName || "(sin nombre)";
    } else if (name) {
      name.remove();
      name = null;
    }
  };

  update(instance, showNames, iconUrl);

  return { element: item, update };
}
