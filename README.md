# DofusTabs

Gestor de multicuenta para Dofus: detecta los clientes del juego abiertos, muestra una sidebar con cada personaje (icono de clase incluido) y permite cambiar de cuenta con un clic o con atajos de teclado globales.

Reescritura multiplataforma en **Rust + Tauri 2** del DofusTabs original (C#/WPF, solo Windows).

## Estado por plataforma

| Plataforma | Estado |
|---|---|
| Linux (Hyprland/Wayland) | Funcional y verificado |
| Windows | Implementado; verificación de compilación pendiente en CI |
| Otros compositores Wayland / X11 | No soportado (ver Roadmap) |

## Arquitectura

Hexagonal (ports & adapters), tres capas Rust + frontend:

```
crates/core       Dominio puro: parseo de títulos, clases, rotación, settings.
                  Sin dependencias de OS ni de Tauri. Testeado al 100 % en unit tests.
crates/platform   Ports (traits GameDiscovery, WindowActivator, HotkeyBackend)
                  y adapters por OS:
                    linux/    hyprctl (discovery, focuswindow, binds + FIFO)
                    windows/  EnumWindows, SetForegroundWindow, RegisterHotKey
src-tauri         Orquestación: comandos IPC, watcher de procesos (1 s),
                  despacho de hotkeys, persistencia atómica de settings.
src/              Frontend vanilla TypeScript + CSS (sin frameworks).
```

La regla de dependencias: `core` no depende de nadie; `platform` depende de `core`; `src-tauri` cablea ambos. Portar a otra plataforma = escribir un adapter nuevo.

## Desarrollo

Requisitos: Rust estable, Node 20+, y en Linux `webkit2gtk-4.1`.

```sh
npm install
npm run tauri dev      # desarrollo
npm run tauri build    # binario de release
cargo test --workspace # suite completa de Rust
npm run build          # verificación del frontend
```

## Atajos por defecto

- Siguiente cuenta: `Alt+Tab` · Anterior: `Alt+Shift+Tab` · Principal: `Alt+Home`
- Configurables desde el panel de ajustes; cada cuenta admite un atajo directo propio.

En Linux los atajos se registran como binds de Hyprland mientras la app corre y se eliminan al cerrarla. Cerrar la app con `SIGKILL` omite esa limpieza (los binds quedan hasta recargar Hyprland).

## Configuración

Se guarda en el directorio de configuración de la app (`~/.config/com.dofustabs.app/settings.json` en Linux) con escritura atómica y copia `.bak`.

## Roadmap

- Verificación del backend Windows en CI (`.github/workflows/ci.yml`, job `windows-latest`).
- Hotkeys vía portal XDG GlobalShortcuts, junto con discovery/activación específicos, para compositores Wayland distintos de Hyprland.
- Soporte X11.
- Embedding de la ventana del juego (estilo original de Windows) como feature opcional.
