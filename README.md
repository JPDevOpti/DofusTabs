# DofusTabs

Gestor de multicuenta para Dofus: detecta los clientes del juego abiertos, muestra una sidebar con cada personaje (icono de clase incluido) y permite cambiar de cuenta con un clic o con atajos de teclado globales.

Reescritura multiplataforma en **Rust + Tauri 2** del DofusTabs original (C#/WPF, solo Windows).

## Estado por plataforma

| Plataforma | Estado |
|---|---|
| Linux (Hyprland/Wayland) | Funcional y verificado |
| Windows | Implementado; compilación pendiente de verificar en un equipo Windows |
| Otros compositores Wayland / X11 | No soportado (ver Roadmap) |

## Arquitectura

Hexagonal (ports & adapters), tres capas Rust + frontend:

```
crates/core       Dominio puro: parseo de títulos (window_title.rs), clases,
                  rotación, settings. Sin dependencias de OS ni de Tauri.
                  Testeado al 100 % en unit tests.
crates/platform   Ports (traits GameDiscovery, WindowActivator, HotkeyBackend,
                  WindowManager, GpuInspector, GameLauncher) y adapters por OS:
                    linux/    hyprctl (discovery, activator.rs con focuswindow,
                              hotkeys con binds + FIFO, ventanas flotantes),
                              /proc + DRM (gpu.rs), launcher con PRIME render
                              offload
                    windows/  EnumWindows (discovery.rs), SetForegroundWindow
                              (activator.rs), RegisterHotKey (hotkeys.rs vía
                              win32_keymap.rs), gpu.rs/launcher.rs/
                              window_manager.rs (stubs "unsupported")
src-tauri         Orquestación: comandos IPC, watcher de procesos (1 s) que
                  también resuelve la GPU de cada cliente, despacho de hotkeys,
                  layout de ventanas, persistencia atómica de settings.
src/              Frontend vanilla TypeScript + CSS (sin frameworks).
```

La regla de dependencias: `core` no depende de nadie; `platform` depende de `core`; `src-tauri` cablea ambos. Portar a otra plataforma = escribir un adapter nuevo.

## Desarrollo

Requisitos: Rust estable, Node 20+, y en Linux `webkit2gtk-4.1`.

```sh
npm install
./run.sh               # desarrollo con recarga en caliente (equivale a npm run tauri dev)
npm run tauri build    # binario de release
cargo test --workspace # suite completa de Rust
npm run build          # verificación del frontend
```

## Atajos por defecto

- Siguiente cuenta: `Alt+Tab` · Anterior: `Alt+Shift+Tab` · Principal: `Alt+Home`
- Configurables desde el panel de ajustes; cada cuenta admite un atajo directo propio.

En Linux los atajos se registran como binds de Hyprland mientras la app corre y se eliminan al cerrarla. Cerrar la app con `SIGKILL` omite esa limpieza (los binds quedan hasta recargar Hyprland).

## Ventanas y GPU (Linux/Hyprland)

- La ventana de DofusTabs no tiene decoraciones y al arrancar se vuelve flotante y se convierte en una columna de 64 px a toda la altura del área útil del monitor donde abrió; los juegos se apilan a su derecha; mientras el panel de ajustes o el menú de una cuenta están abiertos la ventana se ensancha temporalmente para mostrarlos.
- Con "Gestionar ventanas del juego" activo, cada cliente nuevo se une a un grupo nativo de Hyprland (pestañas del compositor) en el workspace donde estaba DofusTabs al arrancar; la app reserva una columna de 64 px en ese monitor para que el grupo nunca tape la sidebar y la libera al salir; activar una cuenta (atajo o clic) muestra su pestaña. Recomendación: usar un workspace vacío para los juegos, porque el grupo comparte el espacio con las demás ventanas tileadas del workspace. "Reorganizar ventanas" re-aplica el layout a todas.
- No se usa el fullscreen del compositor a propósito: con `misc:on_focus_under_fullscreen = 1` (valor por defecto en Omarchy) Hyprland lo quitaría cada vez que un cliente del juego recibe el foco.
- Cada cuenta muestra un badge `dGPU` (NVIDIA) o `iGPU` (integrada) según los nodos DRM que el proceso tiene abiertos. Sin badge = todavía no renderiza.
- El botón ▶ abre el Ankama Launcher con las variables PRIME (`__NV_PRIME_RENDER_OFFLOAD`, `__GLX_VENDOR_LIBRARY_NAME=nvidia`, ...) para que el juego use la dedicada. La ruta por defecto es `~/Applications/Dofus.AppImage`, configurable en ajustes.

## Configuración

Se guarda en el directorio de configuración de la app (`~/.config/com.dofustabs.app/settings.json` en Linux) con escritura atómica y copia `.bak`.

## Roadmap

- Verificación de compilación del backend Windows (`cargo check --workspace` en un equipo Windows).
- Hotkeys vía portal XDG GlobalShortcuts, junto con discovery/activación específicos, para compositores Wayland distintos de Hyprland.
- Soporte X11.
- Embedding de la ventana del juego (estilo original de Windows) como feature opcional.
- Inspección de GPU y launcher en Windows (hoy son stubs que reportan `unknown`/`unsupported`).
