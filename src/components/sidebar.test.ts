import { describe, expect, it, vi } from "vitest";
import type { InstanceView } from "../ipc";
import { createSidebar, type SidebarHandlers } from "./sidebar";

function instance(pid: number, overrides: Partial<InstanceView> = {}): InstanceView {
  return {
    pid,
    characterName: `Char${pid}`,
    className: null,
    iconFile: null,
    enabled: true,
    displayOrder: pid,
    hotkey: null,
    isPrimary: false,
    isActive: false,
    gpu: "unknown",
    ...overrides,
  };
}

function noopHandlers(): SidebarHandlers {
  return {
    onRefresh: vi.fn(),
    onToggleShowNames: vi.fn(),
    onActivate: vi.fn(),
    onReorder: vi.fn(),
    onMoveUp: vi.fn(),
    onMoveDown: vi.fn(),
    onSetEnabled: vi.fn(),
    onSetPrimary: vi.fn(),
    onInstanceHotkeyChange: vi.fn(),
    onGlobalHotkeyChange: vi.fn(),
    onToggleManageGameWindows: vi.fn(),
    onArrangeWindows: vi.fn(),
    onLauncherPathChange: vi.fn(),
    onLaunchGame: vi.fn(),
    onSetSidebarExtension: vi.fn(),
  };
}

describe("sidebar keyed rendering", () => {
  it("keeps the same DOM element for a pid across re-renders", () => {
    const root = document.createElement("div");
    const sidebar = createSidebar(root, noopHandlers());

    sidebar.update({ instances: [instance(1), instance(2)], showNames: false, settings: null });
    const list = root.querySelector(".accounts") as HTMLElement;
    const first = list.children[0] as Element;
    const second = list.children[1] as Element;

    sidebar.update({
      instances: [instance(1, { isActive: true }), instance(2)],
      showNames: false,
      settings: null,
    });

    expect(list.children[0]).toBe(first);
    expect(list.children[1]).toBe(second);
    expect(first.classList.contains("active")).toBe(true);
  });

  it("reorders existing elements without recreating them", () => {
    const root = document.createElement("div");
    const sidebar = createSidebar(root, noopHandlers());

    sidebar.update({
      instances: [instance(1, { displayOrder: 0 }), instance(2, { displayOrder: 1 })],
      showNames: false,
      settings: null,
    });
    const list = root.querySelector(".accounts") as HTMLElement;
    const first = list.children[0] as Element;
    const second = list.children[1] as Element;

    sidebar.update({
      instances: [instance(1, { displayOrder: 2 }), instance(2, { displayOrder: 1 })],
      showNames: false,
      settings: null,
    });

    expect(list.children[0]).toBe(second);
    expect(list.children[1]).toBe(first);
  });

  it("removes elements for vanished pids", () => {
    const root = document.createElement("div");
    const sidebar = createSidebar(root, noopHandlers());

    sidebar.update({ instances: [instance(1), instance(2)], showNames: false, settings: null });
    const list = root.querySelector(".accounts") as HTMLElement;
    expect(list.children.length).toBe(2);

    sidebar.update({ instances: [instance(1)], showNames: false, settings: null });
    expect(list.children.length).toBe(1);
  });
});
