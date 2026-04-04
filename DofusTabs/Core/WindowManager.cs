using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;

namespace DofusTabs.Core
{
    public class WindowManager
    {
        private sealed class PreviousWindowState
        {
            public int DisplayOrder { get; init; }
            public bool IsEnabled { get; init; }
            public string IndividualHotkey { get; init; } = string.Empty;
        }

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc enumProc, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);

        [DllImport("user32.dll")]
        private static extern int GetWindowTextLength(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern IntPtr GetForegroundWindow();

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll")]
        private static extern bool IsIconic(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);


        private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        private const int SW_RESTORE = 9;
        private static readonly string[] DofusProcessNames = { "dofus", "dofustouch" };
        private static readonly string[] IgnoredWindowTitleTokens =
        {
            "ankama launcher",
            "updater",
            "launcher"
        };

        private List<WindowInfo> _dofusWindows;

        public WindowManager()
        {
            _dofusWindows = new List<WindowInfo>();
        }

        public List<WindowInfo> GetDofusWindows(bool preservePreviousState = true)
        {
            Dictionary<uint, PreviousWindowState> existingWindows = preservePreviousState
                ? _dofusWindows
                    .GroupBy(w => w.ProcessId)
                    .ToDictionary(
                        g => g.Key,
                        g =>
                        {
                            var previous = g.OrderBy(w => w.DisplayOrder).First();
                            return new PreviousWindowState
                            {
                                DisplayOrder = previous.DisplayOrder,
                                IsEnabled = previous.IsEnabled,
                                IndividualHotkey = previous.IndividualHotkey
                            };
                        })
                : new Dictionary<uint, PreviousWindowState>();

            _dofusWindows.Clear();
            EnumWindows(EnumWindowCallback, IntPtr.Zero);
            
            // Restaurar orden, estado y atajos
            // Nota: para ventanas nuevas, asignar un DisplayOrder secuencial sin duplicados.
            var nextDisplayOrder = existingWindows.Any()
                ? existingWindows.Values.Max(w => w.DisplayOrder) + 1
                : 0;

            foreach (var window in _dofusWindows)
            {
                if (preservePreviousState && existingWindows.ContainsKey(window.ProcessId))
                {
                    var existing = existingWindows[window.ProcessId];
                    window.DisplayOrder = existing.DisplayOrder;
                    window.IsEnabled = existing.IsEnabled;
                    window.IndividualHotkey = existing.IndividualHotkey;
                }
                else
                {
                    window.DisplayOrder = nextDisplayOrder++;
                }
            }
            
            return _dofusWindows.OrderBy(w => w.DisplayOrder).ThenBy(w => w.Title).ToList();
        }

        private bool EnumWindowCallback(IntPtr hWnd, IntPtr lParam)
        {
            if (!IsWindowVisible(hWnd))
                return true;

            GetWindowThreadProcessId(hWnd, out uint processId);
            
            try
            {
                Process process = Process.GetProcessById((int)processId);
                string processName = process.ProcessName;

                string processNameWithoutExt = processName.Replace(".exe", "");
                if (IsDofusProcessName(processNameWithoutExt))
                {
                    string title = GetWindowTitle(hWnd);
                    if (!IsRelevantDofusTitle(title))
                    {
                        return true;
                    }

                    var existingWindow = _dofusWindows.FirstOrDefault(w => w.ProcessId == processId);
                    if (existingWindow != null)
                    {
                        // Quedarse con la ventana más descriptiva por proceso evita duplicados y mejora el nombre detectado.
                        if (title.Length > existingWindow.Title.Length)
                        {
                            existingWindow.Handle = hWnd;
                            existingWindow.Title = title;
                            existingWindow.ProcessName = processName;
                        }

                        return true;
                    }

                    _dofusWindows.Add(new WindowInfo
                    {
                        Handle = hWnd,
                        Title = title,
                        ProcessId = processId,
                        ProcessName = processName
                    });
                }
            }
            catch
            {
                // Ignorar procesos que ya no existen o no se pueden acceder
            }

            return true;
        }

        private static bool IsDofusProcessName(string processName)
        {
            return DofusProcessNames.Any(n => processName.Equals(n, StringComparison.OrdinalIgnoreCase));
        }

        private static string GetWindowTitle(IntPtr hWnd)
        {
            int length = GetWindowTextLength(hWnd);
            if (length <= 0)
            {
                return string.Empty;
            }

            StringBuilder windowTitle = new StringBuilder(length + 1);
            GetWindowText(hWnd, windowTitle, windowTitle.Capacity);
            return windowTitle.ToString().Trim();
        }

        private static bool IsRelevantDofusTitle(string title)
        {
            if (string.IsNullOrWhiteSpace(title))
            {
                return false;
            }

            string normalized = NormalizeForMatch(title);
            if (IgnoredWindowTitleTokens.Any(t => normalized.Contains(t, StringComparison.OrdinalIgnoreCase)))
            {
                return false;
            }

            return true;
        }

        private static string NormalizeForMatch(string value)
        {
            string normalized = value.Normalize(NormalizationForm.FormD);
            var sb = new StringBuilder(normalized.Length);

            foreach (char c in normalized)
            {
                if (CharUnicodeInfo.GetUnicodeCategory(c) == UnicodeCategory.NonSpacingMark)
                {
                    continue;
                }

                if (char.IsLetterOrDigit(c) || char.IsWhiteSpace(c))
                {
                    sb.Append(char.ToLowerInvariant(c));
                }
                else
                {
                    sb.Append(' ');
                }
            }

            return Regex.Replace(sb.ToString(), "\\s+", " ").Trim();
        }

        public bool SwitchToNextWindow()
        {
            var windows = GetDofusWindows();
            // Ordenar por DisplayOrder y filtrar solo las habilitadas
            var enabledWindows = windows.Where(w => w.IsEnabled)
                                       .OrderBy(w => w.DisplayOrder)
                                       .ToList();
            
            if (enabledWindows.Count == 0)
                return false;

            // Obtener la ventana activa actual del sistema
            IntPtr activeHandle = GetForegroundWindow();
            var currentWindow = enabledWindows.FirstOrDefault(w => w.Handle == activeHandle);
            
            // Si no encontramos la ventana actual, empezar por la primera
            int currentIndex = currentWindow != null 
                ? enabledWindows.IndexOf(currentWindow) 
                : -1;

            int nextIndex = (currentIndex + 1) % enabledWindows.Count;
            var nextWindow = enabledWindows[nextIndex];
            
            // Encontrar el índice real en la lista completa
            int realIndex = windows.IndexOf(nextWindow);
            return SwitchToWindow(nextWindow, realIndex);
        }

        public bool SwitchToPreviousWindow()
        {
            var windows = GetDofusWindows();
            // Ordenar por DisplayOrder y filtrar solo las habilitadas
            var enabledWindows = windows.Where(w => w.IsEnabled)
                                       .OrderBy(w => w.DisplayOrder)
                                       .ToList();
            
            if (enabledWindows.Count == 0)
                return false;

            // Obtener la ventana activa actual del sistema
            IntPtr activeHandle = GetForegroundWindow();
            var currentWindow = enabledWindows.FirstOrDefault(w => w.Handle == activeHandle);
            
            // Si no encontramos la ventana actual, empezar por la última
            int currentIndex = currentWindow != null 
                ? enabledWindows.IndexOf(currentWindow) 
                : 0;

            int nextIndex = (currentIndex - 1 + enabledWindows.Count) % enabledWindows.Count;
            var nextWindow = enabledWindows[nextIndex];
            
            // Encontrar el índice real en la lista completa
            int realIndex = windows.IndexOf(nextWindow);
            return SwitchToWindow(nextWindow, realIndex);
        }

        public bool SwitchToWindow(WindowInfo windowInfo, int index)
        {
            try
            {
                if (IsIconic(windowInfo.Handle))
                {
                    ShowWindow(windowInfo.Handle, SW_RESTORE);
                }

                SetForegroundWindow(windowInfo.Handle);
                return true;
            }
            catch
            {
                return false;
            }
        }

        public WindowInfo? GetCurrentActiveWindow()
        {
            IntPtr activeHandle = GetForegroundWindow();
            var windows = GetDofusWindows();
            return windows.FirstOrDefault(w => w.Handle == activeHandle);
        }
    }

    public class WindowInfo : System.ComponentModel.INotifyPropertyChanged
    {
        private bool _isEnabled = true;
        private bool _isActive = false;
        private string _individualHotkey = string.Empty;
        private int _displayOrder = 0;

        private static readonly Dictionary<string, string> CharacterClassAliases = BuildClassAliasMap();

        private static readonly string[] IgnoredNameTokens =
        {
            "dofus",
            "touch",
            "ankama",
            "launcher"
        };

        private static Dictionary<string, string> BuildClassAliasMap()
        {
            var aliases = new (string Alias, string ClassName)[]
            {
                ("aniripsa", "Aniripsa"),
                ("anutrof", "Anutrof"),
                ("feca", "Feca"),
                ("forjalanza", "Forjalanza"),
                ("hipermago", "Hipermago"),
                ("ocra", "Ocra"),
                ("osamodas", "Osamodas"),
                ("pandawa", "Pandawa"),
                ("sacrogrito", "Sacrógrito"),
                ("sadida", "Sadida"),
                ("selotrop", "Selotrop"),
                ("sram", "Sram"),
                ("steamer", "Steamer"),
                ("tymador", "Tymador"),
                ("uginak", "Uginak"),
                ("xelor", "Xelor"),
                ("yopuka", "Yopuka"),
                ("zobal", "Zobal"),
                ("zurcar", "Zurcar")
            };

            var duplicateAliases = aliases
                .GroupBy(x => x.Alias, StringComparer.OrdinalIgnoreCase)
                .Where(g => g.Count() > 1)
                .Select(g => g.Key)
                .OrderBy(x => x)
                .ToList();

            if (duplicateAliases.Count > 0)
            {
                throw new InvalidOperationException(
                    $"Aliases de clase duplicados detectados: {string.Join(", ", duplicateAliases)}");
            }

            return aliases.ToDictionary(x => x.Alias, x => x.ClassName, StringComparer.OrdinalIgnoreCase);
        }

        public IntPtr Handle { get; set; }
        public string Title { get; set; } = string.Empty;
        public uint ProcessId { get; set; }
        public string ProcessName { get; set; } = string.Empty;

        public int DisplayOrder
        {
            get => _displayOrder;
            set
            {
                if (_displayOrder != value)
                {
                    _displayOrder = value;
                    OnPropertyChanged(nameof(DisplayOrder));
                }
            }
        }

        public bool IsEnabled
        {
            get => _isEnabled;
            set
            {
                if (_isEnabled != value)
                {
                    _isEnabled = value;
                    OnPropertyChanged(nameof(IsEnabled));
                }
            }
        }

        public bool IsActive
        {
            get => _isActive;
            set
            {
                if (_isActive != value)
                {
                    _isActive = value;
                    OnPropertyChanged(nameof(IsActive));
                }
            }
        }

        public string CharacterName
        {
            get
            {
                var (name, _) = ParseCharacterMetadata();
                return name;
            }
        }

        public string AvatarInitial
        {
            get
            {
                if (string.IsNullOrWhiteSpace(CharacterName))
                    return "?";

                return CharacterName.Substring(0, 1).ToUpperInvariant();
            }
        }

        public string CharacterClass
        {
            get
            {
                var (_, className) = ParseCharacterMetadata();
                return className;
            }
        }

        public string IconPath
        {
            get
            {
                var className = CharacterClass;
                if (string.IsNullOrEmpty(className))
                    return string.Empty;

                // Normalizar el nombre de la clase para buscar el icono
                className = className.Trim();
                
                // Mapeo de nombres de clases a archivos de iconos
                var iconMapping = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
                {
                    { "Aniripsa", "Aniripsa.jpg" },
                    { "Anutrof", "Anutrof.jpg" },
                    { "Feca", "Feca.jpg" },
                    { "Forjalanza", "Forjalanza.png" },
                    { "Hipermago", "Hipermago.jpg" },
                    { "Ocra", "Ocra.jpg" },
                    { "Osamodas", "Osamodas.jpg" },
                    { "Pandawa", "Pandawa.jpg" },
                    { "Sacrógrito", "Sacrgrito.jpg" },
                    { "Sacrogrito", "Sacrgrito.jpg" },
                    { "Sadida", "Sadida.jpg" },
                    { "Selotrop", "Selotrop.jpg" },
                    { "Sram", "Sram.jpg" },
                    { "Steamer", "Steamer.jpg" },
                    { "Tymador", "Tymador.jpg" },
                    { "Uginak", "Uginak.jpg" },
                    { "Xelor", "Xelor.jpg" },
                    { "Yopuka", "Yopuka.jpg" },
                    { "Zobal", "Zobal.jpg" },
                    { "Zurcar", "Zurcar.jpg" }
                };

                if (iconMapping.TryGetValue(className, out var iconFile))
                {
                    // Intentar varias rutas posibles
                    var possiblePaths = new List<string>();
                    
                    // Ruta relativa desde el directorio de trabajo
                    possiblePaths.Add(System.IO.Path.Combine("Resources", "Icons", iconFile));
                    
                    // Ruta desde el directorio de la aplicación (compatible con publish single-file)
                    var appDirectory = AppContext.BaseDirectory;
                    possiblePaths.Add(System.IO.Path.Combine(appDirectory, "Resources", "Icons", iconFile));
                    
                    // Ruta desde el directorio base del proyecto (para desarrollo)
                    var baseDirectory = System.IO.Directory.GetCurrentDirectory();
                    possiblePaths.Add(System.IO.Path.Combine(baseDirectory, "Resources", "Icons", iconFile));
                    possiblePaths.Add(System.IO.Path.Combine(baseDirectory, "DofusTabs", "Resources", "Icons", iconFile));
                    
                    foreach (var path in possiblePaths)
                    {
                        if (System.IO.File.Exists(path))
                        {
                            return System.IO.Path.GetFullPath(path);
                        }
                    }
                }

                return string.Empty;
            }
        }

        public string IndividualHotkey
        {
            get => _individualHotkey;
            set
            {
                if (_individualHotkey != value)
                {
                    _individualHotkey = value;
                    OnPropertyChanged(nameof(IndividualHotkey));
                }
            }
        }

        public string DisplayTitle
        {
            get
            {
                if (string.IsNullOrEmpty(Title))
                    return Title;

                string[] parts = Title.Split(new[] { " - " }, StringSplitOptions.RemoveEmptyEntries);
                
                if (parts.Length >= 2)
                {
                    return $"{parts[1]} - {parts[0]}";
                }
                
                return Title;
            }
        }

        private (string name, string className) ParseCharacterMetadata()
        {
            if (string.IsNullOrWhiteSpace(Title))
            {
                return (string.Empty, string.Empty);
            }

            var tokens = Regex.Split(Title, "\\s*(?:-|\\||–|—)\\s*")
                .Select(t => t.Trim())
                .Where(t => !string.IsNullOrWhiteSpace(t))
                .ToList();

            string className = string.Empty;

            foreach (var token in tokens)
            {
                var resolved = ResolveCharacterClass(token);
                if (!string.IsNullOrEmpty(resolved))
                {
                    className = resolved;
                    break;
                }
            }

            if (string.IsNullOrEmpty(className))
            {
                className = ResolveCharacterClass(Title);
            }

            string name = tokens.FirstOrDefault(t => IsValidCharacterNameToken(t, className)) ?? string.Empty;
            if (string.IsNullOrWhiteSpace(name) && tokens.Count > 0)
            {
                name = tokens[0];
            }

            return (name, className ?? string.Empty);
        }

        private static bool IsValidCharacterNameToken(string token, string className)
        {
            string normalized = NormalizeForMatch(token);
            if (string.IsNullOrWhiteSpace(normalized))
            {
                return false;
            }

            if (IgnoredNameTokens.Any(t => normalized.Contains(t, StringComparison.OrdinalIgnoreCase)))
            {
                return false;
            }

            var resolvedClass = ResolveCharacterClass(token);
            if (!string.IsNullOrEmpty(resolvedClass))
            {
                return false;
            }

            if (!string.IsNullOrEmpty(className) && token.Equals(className, StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }

            return true;
        }

        private static string ResolveCharacterClass(string text)
        {
            string normalizedText = NormalizeForMatch(text);
            if (string.IsNullOrWhiteSpace(normalizedText))
            {
                return string.Empty;
            }

            foreach (var alias in CharacterClassAliases)
            {
                if (ContainsWholeWord(normalizedText, alias.Key))
                {
                    return alias.Value;
                }
            }

            return string.Empty;
        }

        private static bool ContainsWholeWord(string text, string word)
        {
            return Regex.IsMatch(text, $@"(^|\\s){Regex.Escape(word)}($|\\s)", RegexOptions.IgnoreCase);
        }

        private static string NormalizeForMatch(string value)
        {
            string normalized = value.Normalize(NormalizationForm.FormD);
            var sb = new StringBuilder(normalized.Length);

            foreach (char c in normalized)
            {
                if (CharUnicodeInfo.GetUnicodeCategory(c) == UnicodeCategory.NonSpacingMark)
                {
                    continue;
                }

                if (char.IsLetterOrDigit(c) || char.IsWhiteSpace(c))
                {
                    sb.Append(char.ToLowerInvariant(c));
                }
                else
                {
                    sb.Append(' ');
                }
            }

            return Regex.Replace(sb.ToString(), "\\s+", " ").Trim();
        }

        public event System.ComponentModel.PropertyChangedEventHandler? PropertyChanged;

        protected virtual void OnPropertyChanged(string propertyName)
        {
            PropertyChanged?.Invoke(this, new System.ComponentModel.PropertyChangedEventArgs(propertyName));
        }
    }
}

