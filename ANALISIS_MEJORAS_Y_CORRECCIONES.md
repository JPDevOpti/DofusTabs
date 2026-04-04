# Analisis Integral y Plan de Mejoras - DofusTabs

Fecha: 2026-04-04  
Proyecto: DofusTabs  
Branch: main

## 1) Objetivo del documento

Este documento resume el analisis completo del proyecto y define:

- problemas funcionales actuales
- funcionalidades faltantes
- correcciones necesarias
- mejoras de arquitectura y calidad
- roadmap recomendado por fases

El objetivo es tener una guia de ejecucion clara para cerrar la brecha entre lo implementado y lo esperado por producto y README.

## 2) Estado actual (resumen ejecutivo)

- Build de solucion: OK (`dotnet build DofusTabs.sln -c Debug`).
- Runtime (`dotnet run`) y watch (`dotnet watch run`): fallan en arranque por error de inicializacion WPF.
- Arquitectura base: buena direccion (Domain/Application/Infrastructure + DI).
- Funcionalidad visible al usuario: parcial (servicios robustos, UI incompleta para varias features).

## 3) Hallazgos criticos y correcciones requeridas

### 3.1 Bloqueador de arranque (critico)

Problema:

- `App.xaml` usa `StartupUri="UI/MainWindow.xaml"`.
- `App.xaml.cs` tambien crea y muestra `MainWindow` por DI.
- `MainWindow` no tiene constructor vacio, WPF intenta construirla por XAML y lanza `XamlParseException`.

Evidencia:

- `DofusTabs/App.xaml`
- `DofusTabs/App.xaml.cs`
- `%AppData%/DofusTabs/logs/app-YYYY-MM-DD.log`

Correccion:

- Quitar `StartupUri` de `App.xaml` y dejar un unico flujo de arranque por DI.

Resultado esperado:

- `dotnet run --project DofusTabs/DofusTabs.csproj` inicia sin crash.
- `dotnet watch run` deja de entrar en ciclo de crash y lock de ejecutable.

### 3.2 Overlay no integrado (alto)

Problema:

- `OverlayWindow` existe (XAML + code-behind), pero no se instancia ni conecta desde `MainWindow`.
- Se guardan `OverlayVisible`, `OverlayCompact`, `OverlayX`, `OverlayY`, pero no se usan en flujo real.

Evidencia:

- `DofusTabs/UI/OverlayWindow.xaml`
- `DofusTabs/UI/OverlayWindow.xaml.cs`
- `DofusTabs/Application/Settings/AppSettings.cs`
- `DofusTabs/UI/MainWindow.xaml.cs` (sin uso de `OverlayWindow`)

Correccion:

- Integrar `OverlayWindow` al ciclo de vida de `MainWindow`.
- Restaurar y persistir estado de visibilidad, modo compacto y posicion.
- Agregar boton mostrar/ocultar overlay en UI principal.

### 3.3 Configuracion de hotkeys incompleta en UI (alto)

Problema:

- Hotkeys globales e individuales existen en infraestructura, pero falta UI completa para editar y capturar.

Evidencia:

- `DofusTabs/Infrastructure/Hotkeys/HotkeyService.cs`
- `DofusTabs/UI/MainWindow.xaml`
- README promete configuracion interactiva.

Correccion:

- Crear panel de configuracion de hotkeys.
- Implementar modo captura con suspension temporal (`SetSuspended`).

### 3.4 Gestion de cuentas incompleta en UI (alto)

Problema:

- Backend soporta `IsEnabled` y `DisplayOrder`, pero UI no expone flujo completo para habilitar/deshabilitar y reordenar.

Evidencia:

- `DofusTabs/Domain/GameInstance.cs`
- `DofusTabs/Infrastructure/Discovery/GameDiscoveryService.cs`
- `DofusTabs/UI/MainWindow.xaml`

Correccion:

- Agregar UI para toggle por cuenta.
- Agregar reordenado (drag-drop o botones subir/bajar).
- Persistir cambios inmediatamente en settings.

### 3.5 Boton Ajustes en placeholder (alto)

Problema:

- `SettingsButton_Click` solo muestra MessageBox: "Panel de configuracion en construccion".

Correccion:

- Implementar ventana o panel real de ajustes conectado con `ISettingsService`.

## 4) Desalineaciones entre README y estado real

| Tema | README | Estado real | Que hacer |
| --- | --- | --- | --- |
| Overlay flotante completo | Declarado | Parcial/no integrado | Integrar `OverlayWindow` y controles de visibilidad/compacto |
| Atajos configurables por UI | Declarado | Parcial | Implementar editor de hotkeys globales e individuales |
| Reordenamiento drag-drop | Declarado | No visible en UI actual | Implementar reordenamiento con persistencia |
| Habilitar/deshabilitar cuentas | Declarado | Dominio existe, UI incompleta | Exponer toggles por cuenta |
| Estructura de proyecto | Menciona Core/Utils legacy | Repo migrado a capas nuevas | Actualizar README con estructura real |

## 5) Mejoras de arquitectura recomendadas

### 5.1 Reducir acoplamiento a implementaciones concretas

Problema:

- Hay casts a clases concretas (`GameDiscoveryService`, `EmbeddingRegistry`) en lugar de depender de interfaces.

Que hacer:

- Mover metodos necesarios a interfaces o crear interfaces especificas.
- Evitar casts en `MainWindow` y `EmbeddingService`.

### 5.2 Mover logica de UI a ViewModel (MVVM incremental)

Problema:

- `MainWindow.xaml.cs` concentra discovery, embedding, watcher, tray, cierre y settings.

Que hacer:

- Introducir `MainWindowViewModel` por fases.
- Mantener code-behind para eventos visuales y de interop.

### 5.3 Mejorar observabilidad

Problema:

- Existen `catch {}` en rutas sensibles que silencian causas.

Que hacer:

- Mantener captura defensiva cuando haga falta, pero registrar `AppLogger.Warn/Error` en rutas criticas.

## 6) Riesgos operativos y mitigacion

| Riesgo | Impacto | Mitigacion |
| --- | --- | --- |
| Crash en arranque por doble startup | Critico | Quitar `StartupUri` y usar solo DI |
| Lock de `DofusTabs.exe` en watch | Alto | Corregir crash de arranque y limpiar procesos colgados |
| Ventanas embebidas huerfanas tras fallo | Alto | Mantener `RecoveryService` y ampliar logging |
| Configuracion no guardada por errores de I/O | Medio | Validar JSON antes de replace y log visible |
| Regresiones de UX por cambios grandes | Medio | Entregas por fases pequenas con checklist QA |

## 7) Plan recomendado (que se deberia hacer)

### Fase 0 - Estabilizacion inmediata (prioridad maxima)

1. Corregir conflicto de startup en `App.xaml`.
2. Verificar `dotnet run` OK.
3. Verificar `dotnet watch` estable por 10-15 minutos.
4. Ajustar branding residual `DofusMaster` -> `DofusTabs`.

Criterio de salida:

- La app abre y se mantiene viva sin excepciones no controladas.

### Fase 1 - Completar funcionalidad visible

1. Integrar `OverlayWindow` con `MainWindow`.
2. Implementar mostrar/ocultar overlay y modo compacto.
3. Persistir/restaurar estado de overlay (visible, compacto, posicion).
4. Implementar panel de ajustes funcional (sin placeholder).
5. Implementar UI de hotkeys globales e individuales.

Criterio de salida:

- El usuario configura overlay y hotkeys sin tocar archivos ni codigo.

### Fase 2 - Gestion avanzada de cuentas

1. UI para enable/disable por cuenta.
2. UI para reordenado de cuentas (drag-drop o controles de orden).
3. Persistencia inmediata de estado y orden.
4. Sincronizacion exacta entre lista principal y overlay.

Criterio de salida:

- Flujo multicuenta completo y consistente con README.

### Fase 3 - Hardening de arquitectura y calidad

1. Reducir casts a concretos, usar interfaces completas.
2. Extraer ViewModel principal (MVVM incremental).
3. Mejorar logging de rutas criticas (discovery/hotkeys/settings/recovery).
4. Agregar pruebas automatizadas minimas.

Criterio de salida:

- Mejor mantenibilidad, testabilidad y menor riesgo de regresiones.

## 8) Backlog tecnico puntual (accionable)

1. `DofusTabs/App.xaml`: remover `StartupUri`.
2. `DofusTabs/App.xaml.cs`: mantener un unico flujo de creacion de ventana por DI.
3. `DofusTabs/UI/MainWindow.xaml.cs`: integrar instancia de `OverlayWindow`.
4. `DofusTabs/UI/MainWindow.xaml`: agregar boton/toggle de overlay.
5. `DofusTabs/UI/MainWindow.xaml.cs`: reemplazar `SettingsButton_Click` por ventana real.
6. `DofusTabs/UI/MainWindow.xaml`: agregar captura de hotkeys globales.
7. `DofusTabs/UI/MainWindow.xaml.cs`: captura segura de hotkeys con `SetSuspended`.
8. `DofusTabs/UI/MainWindow.xaml`: agregar controles de enable/disable por cuenta.
9. `DofusTabs/UI/MainWindow.xaml`: agregar controles de orden (o drag-drop).
10. `DofusTabs/Infrastructure/Settings/SettingsService.cs`: validar JSON serializado antes de persistir.
11. `DofusTabs/Infrastructure/Discovery/GameDiscoveryService.cs`: exponer por interfaz lo necesario y eliminar cast en UI.
12. `DofusTabs/Infrastructure/Embedding/EmbeddingService.cs`: evitar dependencia implicita de tipo concreto de registry.
13. `README.md`: actualizar funcionalidades y estructura real del proyecto.

## 9) Pruebas minimas recomendadas antes de release

### Smoke

1. Abrir app sin clientes Dofus (sin crash).
2. Abrir 2-4 clientes Dofus y refrescar (deteccion correcta).
3. Activar cuentas desde sidebar y hotkeys.
4. Mostrar/ocultar overlay y probar click de cambio de cuenta.
5. Cerrar app y verificar recovery/restauracion de ventanas.
6. Reabrir app y confirmar persistencia (orden, enabled, hotkeys, overlay).

### Operativo

1. `dotnet build` x3 consecutivas.
2. `dotnet watch run` sin bucles de error.
3. Sin errores no controlados en logs.

## 10) Estimacion de esfuerzo

- Fase 0: 1-2 horas.
- Fase 1: 6-10 horas.
- Fase 2: 4-8 horas.
- Fase 3: 6-12 horas.

Total estimado: 17-32 horas (segun profundidad de MVVM y cobertura de tests).

## 11) Conclusiones

El proyecto tiene una base tecnica solida para embedding/discovery/hotkeys/settings, pero hoy no entrega toda la funcionalidad prometida al usuario final por faltas de integracion UI y un bug critico de arranque.

Prioridad absoluta:

1. arreglar startup
2. integrar overlay
3. completar panel de ajustes y hotkeys

Con esas tres lineas cerradas, DofusTabs pasa de estado parcial a un MVP funcional y coherente con su propuesta.
