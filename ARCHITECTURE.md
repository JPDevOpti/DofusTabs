# DofusTabs — Arquitectura Propuesta v2

> Documento de diseño. Describe la arquitectura objetivo, los problemas que resuelve y el camino de migración desde el código actual.

---

## 1. Problemas del codebase actual

| # | Problema | Impacto |
|---|---|---|
| P1 | `CharacterClassAliases` e `iconMapping` son dos diccionarios separados con las mismas clases | Agregar una clase nueva requiere editar 2 lugares; fácil de desincronizar |
| P2 | `RecoverAllEmbeddedWindows` hardcodea `"Dofus"` | DofusTouch no se recupera en un crash |
| P3 | `WindowEmbeddingService` no tiene registro de instancias activas | El recovery estático no sabe qué ventanas están realmente embebidas |
| P4 | `OverlayWindow` crea su propio `WindowManager` en lugar de compartir el de `MainWindow` | Doble ciclo de `EnumWindows` por cada tick de 50ms |
| P5 | `WindowInfo` mezcla dominio puro, lógica Win32 (handle), parsing de título e ícono | Una clase hace demasiado; difícil de testear |
| P6 | `SettingsManager` es todo métodos estáticos sin versionado de schema | Migrar la estructura del JSON es imposible sin romper configs viejas |
| P7 | `App.xaml.cs` perdió la lógica de selección de modo | El arranque en modo maestro o clásico está sin implementar |
| P8 | Sin inyección de dependencias | Las dependencias están hardcodeadas con `new`, imposible sustituir ni testear |
| P9 | Sin logging estructurado | No hay forma de diagnosticar bugs en producción |
| P10 | `MainWindow` maneja detección, embedding, bandeja, hotkeys y estado de UI en una sola clase | God object; cualquier cambio toca todo |

---

## 2. Principios de diseño

- **Una sola fuente de verdad** para datos que se repiten (clases del juego, nombres de proceso).
- **Separación de capas**: dominio puro → servicios → infraestructura Win32 → UI.
- **Inyección de dependencias** desde el composition root (`App.xaml.cs`).
- **Interfaces en el borde** de cada capa: los consumidores dependen de abstracciones, no de implementaciones.
- **MVVM** en toda la UI: cero lógica de negocio en code-behind.
- **Logging estructurado** en toda la app, volcado a archivo rotado.
- **Recuperación segura** siempre disponible: en startup, en shutdown, en unhandled exception.

---

## 3. Estructura de directorios objetivo

```
DofusTabs/
│
├── App.xaml                         # Composition root — único lugar con `new`
├── App.xaml.cs
│
├── Domain/                          # Modelos puros. Sin Win32, sin WPF, sin I/O.
│   ├── GameInstance.cs              # Reemplaza WindowInfo (solo datos de dominio)
│   ├── GameClass.cs                 # Fuente única de verdad: nombre, alias, archivo de ícono
│   ├── HotkeyBinding.cs             # Value object: modificadores + tecla
│   └── AppMode.cs                   # Enum: Classic | Master
│
├── Services/                        # Interfaces de negocio. Sin dependencias de infraestructura.
│   ├── IGameDiscoveryService.cs     # Detectar instancias del juego
│   ├── IEmbeddingService.cs         # Embeber/restaurar ventanas
│   ├── IEmbeddingRegistry.cs        # Registro de instancias actualmente embebidas
│   ├── IHotkeyService.cs            # Registrar/disparar hotkeys globales
│   ├── ISettingsService.cs          # Leer/escribir configuración
│   ├── IProcessWatcher.cs           # Observar procesos que aparecen/desaparecen
│   └── IRecoveryService.cs          # Recuperación de emergencia
│
├── Infrastructure/                  # Implementaciones concretas.
│   │
│   ├── Win32/                       # Toda la interop Win32 aislada aquí.
│   │   ├── User32.cs                # Declaraciones P/Invoke (solo estáticos internos)
│   │   ├── WindowEnumerator.cs      # Wrapper sobre EnumWindows
│   │   └── EmbeddingEngine.cs       # SetParent / SetWindowLong / SetWindowPos
│   │
│   ├── GameDiscoveryService.cs      # Implementa IGameDiscoveryService usando Win32/
│   ├── EmbeddingService.cs          # Implementa IEmbeddingService usando EmbeddingEngine
│   ├── EmbeddingRegistry.cs         # Implementa IEmbeddingRegistry (thread-safe)
│   ├── HotkeyService.cs             # Implementa IHotkeyService (RegisterHotKey Win32)
│   ├── ProcessWatcher.cs            # Implementa IProcessWatcher (DispatcherTimer)
│   ├── RecoveryService.cs           # Implementa IRecoveryService (usa registry + EnumWindows fallback)
│   │
│   └── Settings/
│       ├── ISettingsService.cs      # (definida en Services/, implementada aquí)
│       ├── SettingsService.cs       # Lectura/escritura JSON con migración
│       ├── SettingsV1.cs            # Schema versión 1 (el actual)
│       ├── SettingsV2.cs            # Schema versión 2 (con MasterMode, etc.)
│       └── SettingsMigrator.cs      # Pipeline de migraciones V1→V2→...
│
├── UI/
│   ├── Shell/
│   │   ├── ShellViewModel.cs        # Gestiona el modo activo (Classic/Master) y navegación
│   │   └── ShellWindow.xaml         # Ventana contenedora; swapea el contenido según modo
│   │
│   ├── Classic/
│   │   ├── ClassicViewModel.cs      # Estado del modo clásico
│   │   ├── ClassicView.xaml         # Vista del modo clásico (antes MainWindow)
│   │   └── OverlayViewModel.cs      # Estado del overlay flotante
│   │   └── OverlayView.xaml         # Overlay flotante (antes OverlayWindow)
│   │
│   ├── Master/
│   │   ├── MasterViewModel.cs       # Estado del modo maestro
│   │   └── MasterView.xaml          # Vista del modo maestro (embed + sidebar)
│   │
│   └── Shared/
│       ├── Controls/
│       │   ├── AccountBubble.xaml   # Control reutilizable de burbuja de cuenta
│       │   └── HotkeyPicker.xaml    # Control para capturar un hotkey
│       ├── Converters/              # IValueConverter para bindings
│       └── ClassIconResolver.cs     # Resuelve ruta de ícono a partir de GameClass
│
└── Diagnostics/
    └── AppLogger.cs                 # Fachada de logging (wrappea Serilog/MEL)
```

---

## 4. Capa Domain — detalles

### 4.1 `GameClass.cs` — fuente única de verdad

Elimina la duplicación entre `CharacterClassAliases` e `iconMapping`.

```csharp
// Domain/GameClass.cs
public sealed record GameClass(
    string CanonicalName,   // "Sacrógrito"
    string IconFile,        // "Sacrgrito.jpg"
    string[] Aliases        // ["sacrogrito", "sacrógrito"]
)
{
    public static readonly IReadOnlyList<GameClass> All = new[]
    {
        new GameClass("Aniripsa",   "Aniripsa.jpg",   ["aniripsa"]),
        new GameClass("Anutrof",    "Anutrof.jpg",    ["anutrof"]),
        new GameClass("Feca",       "Feca.jpg",       ["feca"]),
        new GameClass("Forjalanza", "Forjalanza.png", ["forjalanza"]),
        new GameClass("Hipermago",  "Hipermago.jpg",  ["hipermago"]),
        new GameClass("Ocra",       "Ocra.jpg",       ["ocra"]),
        new GameClass("Osamodas",   "Osamodas.jpg",   ["osamodas"]),
        new GameClass("Pandawa",    "Pandawa.jpg",     ["pandawa"]),
        new GameClass("Sacrógrito", "Sacrgrito.jpg",  ["sacrogrito", "sacrógrito"]),
        new GameClass("Sadida",     "Sadida.jpg",     ["sadida"]),
        new GameClass("Selotrop",   "Selotrop.jpg",   ["selotrop"]),
        new GameClass("Sram",       "Sram.jpg",       ["sram"]),
        new GameClass("Steamer",    "Steamer.jpg",    ["steamer"]),
        new GameClass("Tymador",    "Tymador.jpg",    ["tymador"]),
        new GameClass("Uginak",     "Uginak.jpg",     ["uginak"]),
        new GameClass("Xelor",      "Xelor.jpg",      ["xelor"]),
        new GameClass("Yopuka",     "Yopuka.jpg",     ["yopuka"]),
        new GameClass("Zobal",      "Zobal.jpg",      ["zobal"]),
        new GameClass("Zurcar",     "Zurcar.jpg",     ["zurcar"]),
    };

    // Lookup rápido por alias (construido una sola vez al arrancar)
    private static readonly Dictionary<string, GameClass> _byAlias =
        All.SelectMany(c => c.Aliases.Select(a => (alias: a, cls: c)))
           .ToDictionary(x => x.alias, x => x.cls, StringComparer.OrdinalIgnoreCase);

    public static GameClass? ResolveFromToken(string token) =>
        _byAlias.TryGetValue(token, out var cls) ? cls : null;
}
```

Agregar una clase nueva = agregar **una línea** en `GameClass.All`. Nada más.

---

### 4.2 `GameInstance.cs` — modelo de dominio limpio

`WindowInfo` actual mezcla el handle Win32 con lógica de negocio. Separar:

```csharp
// Domain/GameInstance.cs
// Datos puros. Sin handle, sin Win32, completamente serializable y testeable.
public sealed class GameInstance : INotifyPropertyChanged
{
    public required uint ProcessId { get; init; }
    public required string WindowTitle { get; init; }
    public required string ProcessName { get; init; }

    // Derivados del título, calculados una sola vez
    public string CharacterName { get; init; } = string.Empty;
    public GameClass? Class { get; init; }

    // Estado mutable (persiste en settings)
    public bool IsEnabled { get; set; } = true;
    public int DisplayOrder { get; set; }
    public HotkeyBinding? IndividualHotkey { get; set; }

    // Estado de UI transitorio (no persiste)
    public bool IsActive { get; set; }

    // INotifyPropertyChanged...
}
```

El handle `IntPtr` vive **únicamente** en `Infrastructure/Win32/` y en los servicios que lo necesitan. El dominio nunca lo ve.

---

### 4.3 `HotkeyBinding.cs` — value object

```csharp
// Domain/HotkeyBinding.cs
public sealed record HotkeyBinding(ModifierKeys Modifiers, Key Key)
{
    public static readonly HotkeyBinding None = new(ModifierKeys.None, Key.None);
    public bool IsEmpty => Key == Key.None;

    public override string ToString()
    {
        var parts = new List<string>();
        if ((Modifiers & ModifierKeys.Control) != 0) parts.Add("Ctrl");
        if ((Modifiers & ModifierKeys.Alt) != 0)     parts.Add("Alt");
        if ((Modifiers & ModifierKeys.Shift) != 0)   parts.Add("Shift");
        parts.Add(Key.ToString());
        return string.Join(" + ", parts);
    }
}
```

---

## 5. Capa Services — interfaces

```csharp
// Services/IGameDiscoveryService.cs
public interface IGameDiscoveryService
{
    // Retorna snapshot actual de instancias. Thread-safe.
    IReadOnlyList<GameInstance> GetInstances();
}

// Services/IEmbeddingRegistry.cs
// Registro centralizado. El RecoveryService lo usa para saber exactamente
// qué ventanas están embebidas sin recurrir a EnumWindows como primera opción.
public interface IEmbeddingRegistry
{
    void Register(uint processId, IntPtr hwnd, EmbeddedWindowSnapshot snapshot);
    void Unregister(uint processId);
    IReadOnlyList<(uint ProcessId, IntPtr Hwnd, EmbeddedWindowSnapshot Snapshot)> GetAll();
    bool TryGet(uint processId, out EmbeddedWindowSnapshot snapshot);
}

// Services/IEmbeddingService.cs
public interface IEmbeddingService
{
    bool TryEmbed(GameInstance instance, IntPtr hostHandle, int width, int height);
    void Restore(uint processId);
    void RestoreAll();
    void Resize(int width, int height);
}

// Services/IRecoveryService.cs
// Siempre seguro de llamar, incluso desde un unhandled exception handler.
public interface IRecoveryService
{
    void RecoverAll();
}

// Services/IHotkeyService.cs
public interface IHotkeyService : IDisposable
{
    bool Register(int id, HotkeyBinding binding);
    void Unregister(int id);
    void UnregisterAll();
    event EventHandler<int> HotkeyFired; // id del hotkey
}

// Services/IProcessWatcher.cs
public interface IProcessWatcher : IDisposable
{
    void Start();
    void Stop();
    event EventHandler<uint> ProcessAppeared;   // processId
    event EventHandler<uint> ProcessDisappeared; // processId
}

// Services/ISettingsService.cs
public interface ISettingsService
{
    AppSettings Load();
    void Save(AppSettings settings);
}
```

---

## 6. Capa Infrastructure — detalles clave

### 6.1 `Win32/User32.cs` — todo el P/Invoke en un solo lugar

```csharp
// Infrastructure/Win32/User32.cs
// internal: nadie fuera de Infrastructure lo importa directamente.
internal static class User32
{
    [DllImport("user32.dll")] internal static extern bool EnumWindows(EnumWindowsProc fn, IntPtr lp);
    [DllImport("user32.dll")] internal static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", SetLastError = true)] internal static extern IntPtr SetParent(IntPtr child, IntPtr newParent);
    [DllImport("user32.dll")] internal static extern IntPtr GetParent(IntPtr hWnd);
    [DllImport("user32.dll")] internal static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] internal static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] internal static extern bool RegisterHotKey(IntPtr hWnd, int id, uint mod, uint vk);
    [DllImport("user32.dll")] internal static extern bool UnregisterHotKey(IntPtr hWnd, int id);
    // ... resto
    internal delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
}
```

Ninguna clase fuera de `Infrastructure/Win32/` tiene `DllImport`. Si en el futuro existe un mock o un backend distinto (ej. para tests), solo hay que cambiar esta capa.

---

### 6.2 `EmbeddingRegistry.cs` — thread-safe

```csharp
// Infrastructure/EmbeddingRegistry.cs
public sealed class EmbeddingRegistry : IEmbeddingRegistry
{
    // ProcessName → ¿es proceso del juego? No hardcodeado: viene de GameDiscoveryService.
    private static readonly HashSet<string> GameProcessNames = new(StringComparer.OrdinalIgnoreCase)
    {
        "dofus", "dofustouch"   // Fuente única. RecoveryService también lee esto.
    };

    private readonly ConcurrentDictionary<uint, EmbeddedEntry> _entries = new();

    public void Register(uint processId, IntPtr hwnd, EmbeddedWindowSnapshot snapshot) =>
        _entries[processId] = new EmbeddedEntry(hwnd, snapshot);

    public void Unregister(uint processId) =>
        _entries.TryRemove(processId, out _);

    public IReadOnlyList<(uint, IntPtr, EmbeddedWindowSnapshot)> GetAll() =>
        _entries.Select(e => (e.Key, e.Value.Hwnd, e.Value.Snapshot)).ToList();

    // Expone los nombres de proceso conocidos para que RecoveryService filtre correctamente
    public static IReadOnlySet<string> KnownProcessNames => GameProcessNames;
}
```

`RecoveryService` usa el registro como primera opción. Si está vacío (ej. crash antes de registrar), cae al fallback de `EnumWindows` filtrando por `KnownProcessNames`, que ya incluye `"dofustouch"`. **Fin del bug P2/P3.**

---

### 6.3 `Settings/SettingsV2.cs` + `SettingsMigrator.cs` — versionado

```csharp
// Infrastructure/Settings/SettingsV2.cs
public sealed class SettingsV2
{
    public int SchemaVersion { get; init; } = 2;
    public HotkeyBindingDto NextHotkey { get; set; } = new("Alt", "Tab");
    public HotkeyBindingDto PreviousHotkey { get; set; } = new("Alt,Shift", "Tab");
    public bool MasterModeEnabled { get; set; } = false;
    public OverlayDto Overlay { get; set; } = new();
    public List<WindowSettingsDto> Windows { get; set; } = [];
}
```

```csharp
// Infrastructure/Settings/SettingsMigrator.cs
// Cada migración es una función pura: JsonDocument → JsonDocument.
// Se encadenan hasta llegar a la versión actual.
public static class SettingsMigrator
{
    private static readonly Dictionary<int, Func<JsonDocument, JsonDocument>> Migrations = new()
    {
        [1] = MigrateV1ToV2,
        // [2] = MigrateV2ToV3, // cuando llegue el momento
    };

    public static JsonDocument MigrateToLatest(JsonDocument doc)
    {
        int version = doc.RootElement.TryGetProperty("SchemaVersion", out var v) ? v.GetInt32() : 1;

        while (Migrations.TryGetValue(version, out var migrate))
        {
            doc = migrate(doc);
            version++;
        }
        return doc;
    }

    private static JsonDocument MigrateV1ToV2(JsonDocument v1)
    {
        // Transformar shape de V1 a V2 sin perder datos del usuario
        // ...
    }
}
```

`SettingsService.Load()` deserializa primero a `JsonDocument`, corre `MigrateToLatest`, luego deserializa a `SettingsV2`. Nunca más se rompen configs de versiones anteriores.

---

### 6.4 `ProcessWatcher.cs` — polling limpio

```csharp
// Infrastructure/ProcessWatcher.cs
// Usa DispatcherTimer (ya tienes esto), pero ahora es un servicio con interfaz.
// ProcessAppeared/ProcessDisappeared son eventos tipados, no callbacks ad-hoc.
public sealed class ProcessWatcher : IProcessWatcher
{
    private readonly IGameDiscoveryService _discovery;
    private readonly DispatcherTimer _timer;
    private HashSet<uint> _lastKnownPids = [];

    public event EventHandler<uint>? ProcessAppeared;
    public event EventHandler<uint>? ProcessDisappeared;

    public ProcessWatcher(IGameDiscoveryService discovery)
    {
        _discovery = discovery;
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _timer.Tick += OnTick;
    }

    public void Start() => _timer.Start();
    public void Stop()  => _timer.Stop();

    private void OnTick(object? s, EventArgs e)
    {
        var current = _discovery.GetInstances().Select(i => i.ProcessId).ToHashSet();
        foreach (var pid in current.Except(_lastKnownPids)) ProcessAppeared?.Invoke(this, pid);
        foreach (var pid in _lastKnownPids.Except(current)) ProcessDisappeared?.Invoke(this, pid);
        _lastKnownPids = current;
    }

    public void Dispose() => _timer.Stop();
}
```

---

## 7. Capa UI — MVVM

### 7.1 Composition root en `App.xaml.cs`

El único lugar donde se instancian las implementaciones concretas.

```csharp
// App.xaml.cs
public partial class App : Application
{
    private ServiceProvider _services = null!;

    protected override void OnStartup(StartupEventArgs e)
    {
        var collection = new ServiceCollection();
        RegisterServices(collection);
        _services = collection.BuildServiceProvider();

        // Recovery de emergencia antes de mostrar cualquier ventana
        _services.GetRequiredService<IRecoveryService>().RecoverAll();

        DispatcherUnhandledException += (_, ex) =>
        {
            _services.GetRequiredService<IRecoveryService>().RecoverAll();
            AppLogger.Error(ex.Exception, "Unhandled dispatcher exception");
        };

        var shell = _services.GetRequiredService<ShellViewModel>();
        var window = new ShellWindow { DataContext = shell };
        window.Show();

        base.OnStartup(e);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _services.GetRequiredService<IRecoveryService>().RecoverAll();
        _services.Dispose();
        base.OnExit(e);
    }

    private static void RegisterServices(IServiceCollection s)
    {
        // Infrastructure
        s.AddSingleton<IEmbeddingRegistry, EmbeddingRegistry>();
        s.AddSingleton<IGameDiscoveryService, GameDiscoveryService>();
        s.AddSingleton<IEmbeddingService, EmbeddingService>();
        s.AddSingleton<IHotkeyService, HotkeyService>();
        s.AddSingleton<IProcessWatcher, ProcessWatcher>();
        s.AddSingleton<ISettingsService, SettingsService>();
        s.AddSingleton<IRecoveryService, RecoveryService>();

        // ViewModels
        s.AddSingleton<ShellViewModel>();
        s.AddTransient<ClassicViewModel>();
        s.AddTransient<MasterViewModel>();
        s.AddTransient<OverlayViewModel>();
    }
}
```

---

### 7.2 `ShellViewModel` — gestiona el modo activo

```csharp
// UI/Shell/ShellViewModel.cs
// Decide qué ViewModel/View está activo. 
// Responde a cambios de modo sin reinicar la app.
public sealed class ShellViewModel : ObservableObject
{
    private readonly ISettingsService _settings;
    private object _currentView = null!;

    public object CurrentView { get => _currentView; private set => SetProperty(ref _currentView, value); }

    public ICommand SwitchToClassicCommand { get; }
    public ICommand SwitchToMasterCommand  { get; }

    public ShellViewModel(
        ISettingsService settings,
        ClassicViewModel classic,
        MasterViewModel master)
    {
        _settings = settings;
        SwitchToClassicCommand = new RelayCommand(() => CurrentView = classic);
        SwitchToMasterCommand  = new RelayCommand(() => CurrentView = master);

        // Arrancar en el modo guardado
        var cfg = settings.Load();
        CurrentView = cfg.MasterModeEnabled ? master : (object)classic;
    }
}
```

`ShellWindow.xaml` tiene un `ContentControl` ligado a `CurrentView`. `DataTemplate` en recursos resuelve automáticamente `ClassicViewModel → ClassicView` y `MasterViewModel → MasterView`. **No hay `if/switch` en code-behind.**

---

### 7.3 `ClassicViewModel` — ejemplo de ViewModel limpio

```csharp
// UI/Classic/ClassicViewModel.cs
public sealed class ClassicViewModel : ObservableObject
{
    private readonly IGameDiscoveryService _discovery;
    private readonly IHotkeyService _hotkeys;
    private readonly ISettingsService _settings;
    private readonly IProcessWatcher _watcher;

    public ObservableCollection<GameInstance> Instances { get; } = [];
    public ICommand RefreshCommand { get; }
    public ICommand ToggleOverlayCommand { get; }

    public ClassicViewModel(
        IGameDiscoveryService discovery,
        IHotkeyService hotkeys,
        ISettingsService settings,
        IProcessWatcher watcher)
    {
        _discovery = discovery;
        _hotkeys   = hotkeys;
        _settings  = settings;
        _watcher   = watcher;

        RefreshCommand       = new RelayCommand(Refresh);
        ToggleOverlayCommand = new RelayCommand(ToggleOverlay);

        _watcher.ProcessAppeared    += (_, _) => Refresh();
        _watcher.ProcessDisappeared += (_, _) => Refresh();
        _hotkeys.HotkeyFired        += OnHotkeyFired;

        LoadSettings();
        _watcher.Start();
        Refresh();
    }

    private void Refresh()
    {
        var fresh = _discovery.GetInstances();
        // Merge inteligente: actualizar los existentes, agregar nuevos, quitar los que ya no están
        // (no reemplazar la colección completa para no romper bindings de UI)
        SyncCollection(Instances, fresh);
    }
    // ...
}
```

`ClassicView.xaml` **solo hace bindings**. Cero lógica en code-behind.

---

## 8. Logging

Agregar `Serilog` (o `Microsoft.Extensions.Logging` con un file sink).

```csharp
// Diagnostics/AppLogger.cs
// Fachada estática para llamar desde cualquier lugar sin DI.
// Internamente usa el logger registrado en el container.
public static class AppLogger
{
    private static ILogger _logger = NullLogger.Instance;

    public static void Initialize(ILogger logger) => _logger = logger;

    public static void Info(string message, params object[] args)  => _logger.LogInformation(message, args);
    public static void Warn(string message, params object[] args)  => _logger.LogWarning(message, args);
    public static void Error(Exception ex, string message)         => _logger.LogError(ex, message);
}
```

Configurar en `App.OnStartup`:
```csharp
var logPath = Path.Combine(
    Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
    "DofusTabs", "logs", "app-.log");

Log.Logger = new LoggerConfiguration()
    .MinimumLevel.Debug()
    .WriteTo.File(logPath, rollingInterval: RollingInterval.Day, retainedFileCountLimit: 7)
    .CreateLogger();
```

Rotación diaria, 7 días de retención. Sin impacto en rendimiento (async sink).

---

## 9. Flujo de datos — resumen

```
App.OnStartup
  └── IRecoveryService.RecoverAll()         // Seguridad ante crash previo
  └── ShellViewModel (lee modo de settings)
        └── ClassicViewModel | MasterViewModel
              └── IProcessWatcher.Start()
                    └── [tick cada 1s]
                          └── IGameDiscoveryService.GetInstances()
                                └── WindowEnumerator (EnumWindows, una sola instancia)
                                      └── GameInstance[] (dominio puro)
                    └── ProcessAppeared / ProcessDisappeared
                          └── ViewModel.Refresh() → UI actualiza
              └── IHotkeyService
                    └── HotkeyFired → ViewModel.OnHotkeyFired()
                          └── IEmbeddingService.TryEmbed() | SetForegroundWindow
                                └── IEmbeddingRegistry.Register()
```

---

## 10. Correcciones específicas por problema

| # | Fix |
|---|---|
| P1 | `GameClass.All` es la fuente única. `CharacterClassAliases` e `iconMapping` eliminados. |
| P2 | `EmbeddingRegistry.KnownProcessNames` contiene `"dofus"` y `"dofustouch"`. `RecoveryService` lo lee. |
| P3 | `IEmbeddingRegistry` rastrea exactamente qué handles están embebidos. `RecoveryService` los usa directo. |
| P4 | `IGameDiscoveryService` es singleton. Un solo `EnumWindows` compartido. `OverlayViewModel` suscribe a `IProcessWatcher`, no crea su propio timer. |
| P5 | `GameInstance` = datos de dominio. El handle `IntPtr` solo existe en `Infrastructure/Win32/`. |
| P6 | `SettingsMigrator` + `SchemaVersion` en JSON. Migración automática al cargar. |
| P7 | `ShellViewModel` gestiona el modo con `ISettingsService`. `App.xaml.cs` es el composition root. |
| P8 | `Microsoft.Extensions.DependencyInjection` en `App.OnStartup`. |
| P9 | Serilog con file sink rotado en `%AppData%\DofusTabs\logs\`. |
| P10 | `MainWindow` dividida en `ShellWindow` (estructura) + `ClassicViewModel` (lógica) + `ClassicView` (UI). |

---

## 11. Dependencias NuGet a agregar

| Paquete | Versión | Para qué |
|---|---|---|
| `Microsoft.Extensions.DependencyInjection` | 8.x | Contenedor de DI |
| `CommunityToolkit.Mvvm` | 8.x | `ObservableObject`, `RelayCommand` (sin boilerplate) |
| `Serilog.Sinks.File` | 5.x | Logging a archivo rotado |
| `Serilog.Extensions.Logging` | 8.x | Integración con MEL si se prefiere |

Total: 4 paquetes ligeros. Sin frameworks pesados.

---

## 12. Plan de migración (incremental, sin romper lo que funciona)

### Fase 1 — Fundación (sin cambios de comportamiento)
- [ ] Crear `Domain/GameClass.cs` con `All` estático. Borrar `CharacterClassAliases` e `iconMapping` de `WindowManager`.
- [ ] Crear `Domain/HotkeyBinding.cs`. Reemplazar `HotkeyConfig` en `HotkeyManager`.
- [ ] Crear `Infrastructure/Win32/User32.cs`. Mover todos los `DllImport` a allí.
- [ ] Crear `Infrastructure/Settings/SettingsV2.cs` + `SettingsMigrator.cs`. Reemplazar `SettingsManager`.

### Fase 2 — DI y logging
- [ ] Agregar NuGet: `Microsoft.Extensions.DependencyInjection`, `CommunityToolkit.Mvvm`, `Serilog.Sinks.File`.
- [ ] Refactorizar `App.xaml.cs` en composition root.
- [ ] Crear `AppLogger` e inicializar Serilog.

### Fase 3 — Servicios
- [ ] Crear interfaces en `Services/`. Mover lógica de `WindowManager` → `GameDiscoveryService`.
- [ ] Crear `EmbeddingRegistry`. Refactorizar `WindowEmbeddingService` → `EmbeddingService` + `RecoveryService`.
- [ ] Crear `ProcessWatcher` como servicio independiente.
- [ ] Crear `HotkeyService` como servicio independiente.

### Fase 4 — MVVM
- [ ] Crear `ShellViewModel` + `ShellWindow`.
- [ ] Crear `ClassicViewModel`. Limpiar `MainWindow.xaml.cs` de lógica.
- [ ] Crear `OverlayViewModel`. Limpiar `OverlayWindow.xaml.cs`.

### Fase 5 — Modo Maestro
- [ ] Crear `MasterViewModel` + `MasterView`.
- [ ] `ShellViewModel` gestiona el switch Classic ↔ Master.
- [ ] Hotkeys funcionales en modo maestro.

### Fase 6 — QA
- [ ] Tests unitarios de `GameClass.ResolveFromToken`, `SettingsMigrator`, `HotkeyBinding`.
- [ ] Tests de integración del ciclo embed → restore.

---

*Cada fase es deployable de forma independiente. El juego sigue funcionando en cada commit.*
