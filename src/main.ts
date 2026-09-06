import {
  ipc,
  onActiveChanged,
  onInstancesChanged,
  type HotkeyView,
  type InstanceView,
  type SettingsView,
} from "./ipc";
import { createSidebar } from "./components/sidebar";

interface AppState {
  instances: InstanceView[];
  showNames: boolean;
  settings: SettingsView | null;
}

const state: AppState = { instances: [], showNames: false, settings: null };

function report(error: unknown): void {
  console.error("[DofusTabs]", error);
}

async function bootstrap(): Promise<void> {
  const root = document.getElementById("app");
  if (!root) throw new Error("Missing #app root element");

  const sidebar = createSidebar(root, {
    onRefresh: () => void ipc.refresh().then(setInstances).catch(report),

    onToggleShowNames: (show) => {
      state.showNames = show;
      update();
      void ipc.setShowNames(show).catch(report);
    },

    onActivate: (pid) => void ipc.activate(pid).catch(report),

    onReorder: (pids) => void ipc.reorder(pids).then(setInstances).catch(report),

    onSetEnabled: (pid, enabled) =>
      void ipc.setEnabled(pid, enabled).then(setInstances).catch(report),

    onSetPrimary: (pid) => void ipc.setPrimary(pid).then(setInstances).catch(report),

    onInstanceHotkeyChange: (pid, hotkey) => {
      const instance = state.instances.find((i) => i.pid === pid);
      if (instance) {
        instance.hotkey = hotkey;
        update();
      }
      void ipc.setInstanceHotkey(pid, hotkey).catch(report);
    },

    onGlobalHotkeyChange: (slot, hotkey) => {
      if (state.settings) {
        const cleared: HotkeyView = { ctrl: false, alt: false, shift: false, superKey: false, key: "" };
        const value = hotkey ?? cleared;
        if (slot === "next") state.settings.nextHotkey = value;
        else if (slot === "previous") state.settings.previousHotkey = value;
        else state.settings.primaryHotkey = value;
      }
      void ipc.setGlobalHotkey(slot, hotkey).catch(report);
    },
  });

  const update = () =>
    sidebar.update({
      instances: state.instances,
      showNames: state.showNames,
      settings: state.settings,
    });

  const setInstances = (instances: InstanceView[]) => {
    state.instances = instances;
    update();
  };

  await onInstancesChanged(setInstances);
  await onActiveChanged((pid) => {
    state.instances = state.instances.map((i) => ({ ...i, isActive: i.pid === pid }));
    update();
  });

  try {
    const settings = await ipc.getSettings();
    state.settings = settings;
    state.showNames = settings.showNames;
  } catch (error) {
    report(error);
  }

  try {
    setInstances(await ipc.refresh());
  } catch (error) {
    report(error);
  }

  update();
}

void bootstrap();
