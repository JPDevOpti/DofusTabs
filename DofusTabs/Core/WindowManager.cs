using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows;

namespace DofusTabs.Core
{
    public class WindowManager
    {
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
        private const string DOFUS_PROCESS_NAME = "Dofus";

        private List<WindowInfo> _dofusWindows;
        private int _currentWindowIndex = -1;

        public WindowManager()
        {
            _dofusWindows = new List<WindowInfo>();
        }

        public List<WindowInfo> GetDofusWindows()
        {
            var existingWindows = _dofusWindows.ToDictionary(w => w.ProcessId, w => new { w.DisplayOrder, w.IsEnabled, w.IndividualHotkey });
            _dofusWindows.Clear();
            EnumWindows(EnumWindowCallback, IntPtr.Zero);
            
            // Restaurar orden, estado y atajos
            foreach (var window in _dofusWindows)
            {
                if (existingWindows.ContainsKey(window.ProcessId))
                {
                    var existing = existingWindows[window.ProcessId];
                    window.DisplayOrder = existing.DisplayOrder;
                    window.IsEnabled = existing.IsEnabled;
                    window.IndividualHotkey = existing.IndividualHotkey;
                }
                else
                {
                    window.DisplayOrder = _dofusWindows.Count;
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

                // Verificar si el proceso es Dofus (comparar sin extensión)
                string processNameWithoutExt = processName.Replace(".exe", "");
                if (processNameWithoutExt.Equals(DOFUS_PROCESS_NAME, StringComparison.OrdinalIgnoreCase))
                {
                    int length = GetWindowTextLength(hWnd);
                    StringBuilder windowTitle = new StringBuilder(length + 1);
                    
                    if (length > 0)
                    {
                        GetWindowText(hWnd, windowTitle, windowTitle.Capacity);
                    }

                    _dofusWindows.Add(new WindowInfo
                    {
                        Handle = hWnd,
                        Title = windowTitle.ToString(),
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

        public bool SwitchToNextWindow()
        {
            var windows = GetDofusWindows();
            // Ordenar por DisplayOrder y filtrar solo las habilitadas
            var enabledWindows = windows.Where(w => w.IsEnabled)
                                       .OrderBy(w => w.DisplayOrder)
                                       .ToList();
            
            if (enabledWindows.Count == 0)
                return false;

            // Encontrar la ventana actual en la lista de habilitadas ordenadas
            var currentWindow = _currentWindowIndex >= 0 && _currentWindowIndex < windows.Count 
                ? windows[_currentWindowIndex] 
                : null;
            
            int currentEnabledIndex = currentWindow != null && currentWindow.IsEnabled
                ? enabledWindows.IndexOf(currentWindow)
                : -1;

            int nextIndex = (currentEnabledIndex + 1) % enabledWindows.Count;
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

            // Encontrar la ventana actual en la lista de habilitadas ordenadas
            var currentWindow = _currentWindowIndex >= 0 && _currentWindowIndex < windows.Count 
                ? windows[_currentWindowIndex] 
                : null;
            
            int currentEnabledIndex = currentWindow != null && currentWindow.IsEnabled
                ? enabledWindows.IndexOf(currentWindow)
                : -1;

            int nextIndex = (currentEnabledIndex - 1 + enabledWindows.Count) % enabledWindows.Count;
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
                _currentWindowIndex = index;
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
            return _dofusWindows.FirstOrDefault(w => w.Handle == activeHandle);
        }
    }

    public class WindowInfo : System.ComponentModel.INotifyPropertyChanged
    {
        private bool _isEnabled = true;
        private string _individualHotkey = string.Empty;
        private int _displayOrder = 0;

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

        public string CharacterName
        {
            get
            {
                if (string.IsNullOrEmpty(Title))
                    return string.Empty;

                string[] parts = Title.Split(new[] { " - " }, StringSplitOptions.RemoveEmptyEntries);
                return parts.Length > 0 ? parts[0] : string.Empty;
            }
        }

        public string CharacterClass
        {
            get
            {
                if (string.IsNullOrEmpty(Title))
                    return string.Empty;

                string[] parts = Title.Split(new[] { " - " }, StringSplitOptions.RemoveEmptyEntries);
                return parts.Length > 1 ? parts[1] : string.Empty;
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
                    { "Sacrígido", "Sacrgrito.jpg" },
                    { "Sacrigido", "Sacrgrito.jpg" },
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
                    
                    // Ruta desde el directorio de la aplicación
                    var appDirectory = System.IO.Path.GetDirectoryName(System.Reflection.Assembly.GetExecutingAssembly().Location);
                    if (appDirectory != null)
                    {
                        possiblePaths.Add(System.IO.Path.Combine(appDirectory, "Resources", "Icons", iconFile));
                    }
                    
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

        public event System.ComponentModel.PropertyChangedEventHandler? PropertyChanged;

        protected virtual void OnPropertyChanged(string propertyName)
        {
            PropertyChanged?.Invoke(this, new System.ComponentModel.PropertyChangedEventArgs(propertyName));
        }
    }
}

