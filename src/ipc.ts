import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface HotkeyView {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  superKey: boolean;
  key: string;
}

/** Which GPU a client renders on, as far as the platform can tell. */
export type GpuKind = "dedicated" | "integrated" | "unknown";

export interface InstanceView {
  pid: number;
  characterName: string;
  className: string | null;
  iconFile: string | null;
  enabled: boolean;
  displayOrder: number;
  hotkey: HotkeyView | null;
  isPrimary: boolean;
  isActive: boolean;
  gpu: GpuKind;
}

export interface SettingsView {
  nextHotkey: HotkeyView;
  previousHotkey: HotkeyView;
  primaryHotkey: HotkeyView;
  showNames: boolean;
  manageGameWindows: boolean;
  /** Effective launcher path (configured or default). */
  launcherPath: string;
}

export type GlobalHotkeySlot = "next" | "previous" | "primary";

export const ipc = {
  refresh: (): Promise<InstanceView[]> => invoke("refresh"),

  activate: (pid: number): Promise<void> => invoke("activate", { pid }),

  setEnabled: (pid: number, enabled: boolean): Promise<InstanceView[]> =>
    invoke("set_enabled", { pid, enabled }),

  reorder: (pids: number[]): Promise<InstanceView[]> => invoke("reorder", { pids }),

  moveUp: (pid: number): Promise<InstanceView[]> => invoke("move_up", { pid }),

  moveDown: (pid: number): Promise<InstanceView[]> => invoke("move_down", { pid }),

  setPrimary: (pid: number): Promise<InstanceView[]> => invoke("set_primary", { pid }),

  getSettings: (): Promise<SettingsView> => invoke("get_settings"),

  setGlobalHotkey: (slot: GlobalHotkeySlot, hotkey: HotkeyView | null): Promise<void> =>
    invoke("set_global_hotkey", { slot, hotkey }),

  setInstanceHotkey: (pid: number, hotkey: HotkeyView | null): Promise<void> =>
    invoke("set_instance_hotkey", { pid, hotkey }),

  setShowNames: (show: boolean): Promise<void> => invoke("set_show_names", { show }),

  setHotkeysSuspended: (suspended: boolean): Promise<void> =>
    invoke("set_hotkeys_suspended", { suspended }),

  setManageGameWindows: (manage: boolean): Promise<void> =>
    invoke("set_manage_game_windows", { manage }),

  arrangeWindows: (): Promise<void> => invoke("arrange_windows"),

  setLauncherPath: (path: string | null): Promise<SettingsView> =>
    invoke("set_launcher_path", { path }),

  launchGame: (): Promise<void> => invoke("launch_game"),
};

export function onInstancesChanged(
  callback: (instances: InstanceView[]) => void,
): Promise<UnlistenFn> {
  return listen<InstanceView[]>("instances-changed", (event) => callback(event.payload));
}

export function onActiveChanged(callback: (pid: number | null) => void): Promise<UnlistenFn> {
  return listen<number | null>("active-changed", (event) => callback(event.payload));
}
