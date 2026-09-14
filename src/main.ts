import { report } from "./components/notice";
import { createSidebar } from "./components/sidebar";
import {
  type HotkeyView,
  type InstanceView,
  ipc,
  onActiveChanged,
  onInstancesChanged,
  type SettingsView,
} from "./ipc";

interface AppState {
  instances: InstanceView[];
  showNames: boolean;
  settings: SettingsView | null;
}

const state: AppState = { instances: [], showNames: false, settings: null };

async function bootstrap(): Promise<void> {
  const root = document.getElementById("app");
  if (!root) throw new Error("Missing #app root element");

  try {
    const { sidebarWidth } = await ipc.layoutConstants();
    document.documentElement.style.setProperty("--sidebar-width", `${sidebarWidth}px`);
  } catch (error) {
    // The command may not exist yet on an older backend; the CSS default
    // keeps the app usable.
    console.warn("[DofusTabs] layout_constants unavailable", error);
  }

  const sidebar = createSidebar(root, {
    onRefresh: () =>
      void ipc
        .refresh()
        .then(setInstances)
        .catch((error) => report(error, "No se pudieron refrescar las cuentas")),

    onToggleShowNames: (show) => {
      const previous = state.showNames;
      state.showNames = show;
      update();
      void ipc.setShowNames(show).catch((error) => {
        state.showNames = previous;
        update();
        report(error, "No se pudo guardar la preferencia de nombres");
      });
    },

    onActivate: (pid) =>
      void ipc.activate(pid).catch((error) => report(error, "No se pudo activar la cuenta")),

    onReorder: (pids) =>
      void ipc
        .reorder(pids)
        .then(setInstances)
        .catch((error) => report(error, "No se pudo reordenar las cuentas")),

    onMoveUp: (pid) =>
      void ipc
        .moveUp(pid)
        .then(setInstances)
        .catch((error) => report(error, "No se pudo mover la cuenta")),

    onMoveDown: (pid) =>
      void ipc
        .moveDown(pid)
        .then(setInstances)
        .catch((error) => report(error, "No se pudo mover la cuenta")),

    onSetEnabled: (pid, enabled) =>
      void ipc
        .setEnabled(pid, enabled)
        .then(setInstances)
        .catch((error) => report(error, "No se pudo cambiar el estado de la cuenta")),

    onSetPrimary: (pid) =>
      void ipc
        .setPrimary(pid)
        .then(setInstances)
        .catch((error) => report(error, "No se pudo marcar la cuenta principal")),

    onInstanceHotkeyChange: (pid, hotkey) => {
      const instance = state.instances.find((i) => i.pid === pid);
      const previous = instance?.hotkey ?? null;
      if (instance) {
        instance.hotkey = hotkey;
        update();
      }
      void ipc.setInstanceHotkey(pid, hotkey).catch((error) => {
        const rollbackInstance = state.instances.find((i) => i.pid === pid);
        if (rollbackInstance) rollbackInstance.hotkey = previous;
        update();
        report(error, "No se pudo guardar el atajo");
      });
    },

    onGlobalHotkeyChange: (slot, hotkey) => {
      const previousSettings = state.settings ? { ...state.settings } : null;
      if (state.settings) {
        const cleared: HotkeyView = {
          ctrl: false,
          alt: false,
          shift: false,
          superKey: false,
          key: "",
        };
        const value = hotkey ?? cleared;
        if (slot === "next") state.settings.nextHotkey = value;
        else if (slot === "previous") state.settings.previousHotkey = value;
        else state.settings.primaryHotkey = value;
        update();
      }
      void ipc.setGlobalHotkey(slot, hotkey).catch((error) => {
        state.settings = previousSettings;
        update();
        report(error, "No se pudo guardar el atajo");
      });
    },

    onToggleManageGameWindows: (manage) => {
      const previous = state.settings?.manageGameWindows ?? false;
      if (state.settings) {
        state.settings.manageGameWindows = manage;
        update();
      }
      void ipc.setManageGameWindows(manage).catch((error) => {
        if (state.settings) state.settings.manageGameWindows = previous;
        update();
        report(error, "No se pudo guardar la preferencia de ventanas");
      });
    },

    onArrangeWindows: () =>
      void ipc
        .arrangeWindows()
        .catch((error) => report(error, "No se pudieron reorganizar las ventanas")),

    onLauncherPathChange: (path) =>
      void ipc
        .setLauncherPath(path)
        .then((settings) => {
          state.settings = settings;
          update();
        })
        .catch((error) => report(error, "No se pudo guardar la ruta del launcher")),

    onLaunchGame: () =>
      void ipc.launchGame().catch((error) => report(error, "No se pudo abrir el launcher")),

    onSetSidebarExtension: (extra) =>
      void ipc.setSidebarExtension(extra).catch((error) => {
        console.warn("[DofusTabs] setSidebarExtension failed", error);
      }),
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
    report(error, "No se pudieron cargar los ajustes");
  }

  try {
    setInstances(await ipc.refresh());
  } catch (error) {
    report(error, "No se pudieron cargar las cuentas");
  }

  update();
}

void bootstrap();
