using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using DofusTabs.Core;
using DofusTabs.Utils;

namespace DofusTabs.UI
{
    public partial class OverlayWindow : Window
    {
        private WindowManager _windowManager;
        private WindowInfo? _currentActiveWindow;
        private DispatcherTimer _checkTimer;
        private List<WindowInfo> _cachedWindows = new List<WindowInfo>();
        private List<WindowInfo> _providedWindows = new List<WindowInfo>();
        private static readonly SolidColorBrush ActiveBrush = new SolidColorBrush(Color.FromRgb(143, 122, 78));
        private static readonly SolidColorBrush ActiveBorderBrush = new SolidColorBrush(Color.FromRgb(111, 90, 52));
        private bool _isCompact = false;
        private Dictionary<uint, Border> _borderCache = new Dictionary<uint, Border>();
        private uint? _lastActiveProcessId = null;

        public event Action? OnOverlayHidden;
        public event Action<bool>? OnCompactChanged;

        public bool IsCompact => _isCompact;

        public OverlayWindow()
        {
            InitializeComponent();
            _windowManager = new WindowManager();
            
            // Timer optimizado para actualizar el resaltado más rápido
            _checkTimer = new DispatcherTimer();
            _checkTimer.Interval = TimeSpan.FromMilliseconds(50);
            _checkTimer.Tick += Timer_Tick;
            
            Loaded += OverlayWindow_Loaded;
            ApplyLayoutMode();
        }

        private void OverlayWindow_Loaded(object sender, RoutedEventArgs e)
        {
            UpdateCharactersList();
            _checkTimer.Start();
        }

        private void CompactToggleButton_Click(object sender, RoutedEventArgs e)
        {
            _isCompact = !_isCompact;
            ApplyLayoutMode();
            HighlightCurrentCharacter();
            OnCompactChanged?.Invoke(_isCompact);
        }

        private void CloseOverlayButton_Click(object sender, RoutedEventArgs e)
        {
            Hide();
            OnOverlayHidden?.Invoke();
        }

        private void Border_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            // Solo permitir drag si se hace clic en el contenedor (no en los items)
            if (e.Source == sender && e.ButtonState == MouseButtonState.Pressed)
            {
                this.DragMove();
            }
        }

        private void CharacterItem_Click(object sender, MouseButtonEventArgs e)
        {
            if (sender is Border border && border.Tag is WindowInfo windowInfo)
            {
                if (windowInfo.IsEnabled)
                {
                    // Cambiar a esta ventana
                    var windows = _cachedWindows.OrderBy(w => w.DisplayOrder).ToList();
                    int index = windows.IndexOf(windowInfo);
                    if (index >= 0)
                    {
                        _windowManager.SwitchToWindow(windowInfo, index);
                    }
                }
                e.Handled = true;
            }
        }

        private void Timer_Tick(object? sender, EventArgs e)
        {
            UpdateActiveWindow();
        }

        private void UpdateActiveWindow()
        {
            // Obtener ventana activa de forma optimizada sin alterar el orden de la lista
            var activeWindow = GetCurrentActiveWindowFast();

            // Solo actualizar resaltado si cambió la ventana activa
            if (activeWindow != null && activeWindow.ProcessId != _currentActiveWindow?.ProcessId)
            {
                _currentActiveWindow = activeWindow;
                HighlightCurrentCharacter();
            }
            else if (activeWindow == null && _currentActiveWindow != null)
            {
                _currentActiveWindow = null;
                HighlightCurrentCharacter();
            }
        }

        public void RefreshWindowsList()
        {
            RefreshWindowsList(null);
        }

        public void RefreshWindowsList(List<WindowInfo>? windowsFromMain = null)
        {
            if (windowsFromMain != null)
            {
                // Guardar la lista proporcionada para mantener el mismo orden
                _providedWindows = windowsFromMain.ToList();
            }

            if (!_providedWindows.Any())
            {
                // Sin lista proporcionada no modificar el orden actual
                return;
            }

            var newWindows = _providedWindows
                .Where(w => w.IsEnabled)
                .ToList();

            bool hasChanged = false;
            if (_cachedWindows.Count != newWindows.Count)
            {
                hasChanged = true;
            }
            else
            {
                for (int i = 0; i < _cachedWindows.Count; i++)
                {
                    if (_cachedWindows[i].ProcessId != newWindows[i].ProcessId)
                    {
                        hasChanged = true;
                        break;
                    }
                }
            }

            if (hasChanged)
            {
                _cachedWindows = newWindows;
                CharactersList.ItemsSource = _cachedWindows;
                
                // Limpiar caché de borders porque los contenedores cambiaron
                _borderCache.Clear();
                _lastActiveProcessId = null;

                if (_currentActiveWindow != null)
                {
                    HighlightCurrentCharacter();
                }
            }
        }

        [DllImport("user32.dll")]
        private static extern IntPtr GetForegroundWindow();

        private WindowInfo? GetCurrentActiveWindowFast()
        {
            // Usar API directa para obtener ventana activa sin recargar toda la lista
            IntPtr activeHandle = GetForegroundWindow();
            return _cachedWindows.FirstOrDefault(w => w.Handle == activeHandle);
        }

        private void UpdateCharactersList()
        {
            // Solo refrescar si ya hay lista provista desde MainWindow
            if (_providedWindows.Any())
            {
                RefreshWindowsList(null);
            }
        }

        private void HighlightCurrentCharacter()
        {
            uint? currentActiveId = _currentActiveWindow?.ProcessId;
            
            // Solo actualizar si realmente cambió
            if (currentActiveId == _lastActiveProcessId)
                return;
            
            uint? previousActiveId = _lastActiveProcessId;
            _lastActiveProcessId = currentActiveId;

            // Actualizar solo los elementos que cambiaron (anterior y nuevo)
            foreach (var item in CharactersList.Items)
            {
                if (item is WindowInfo windowInfo)
                {
                    // Solo procesar si este item es el nuevo activo o era el anterior activo
                    if (windowInfo.ProcessId != currentActiveId && windowInfo.ProcessId != previousActiveId)
                        continue;

                    // Buscar border en caché o en árbol visual
                    if (!_borderCache.TryGetValue(windowInfo.ProcessId, out var border))
                    {
                        var container = CharactersList.ItemContainerGenerator.ContainerFromItem(item) as FrameworkElement;
                        if (container != null)
                        {
                            border = FindVisualChild<Border>(container);
                            if (border != null)
                            {
                                _borderCache[windowInfo.ProcessId] = border;
                            }
                        }
                    }

                    if (border != null)
                    {
                        if (windowInfo.ProcessId == currentActiveId)
                        {
                            border.Background = ActiveBrush;
                            border.BorderBrush = ActiveBorderBrush;
                            border.BorderThickness = new Thickness(1);
                        }
                        else
                        {
                            border.Background = Brushes.Transparent;
                            border.BorderThickness = new Thickness(0);
                        }
                    }
                }
            }
        }

        private static T? FindVisualChild<T>(DependencyObject parent) where T : DependencyObject
        {
            for (int i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
            {
                var child = VisualTreeHelper.GetChild(parent, i);
                if (child is T result)
                    return result;
                
                var childOfChild = FindVisualChild<T>(child);
                if (childOfChild != null)
                    return childOfChild;
            }
            return null;
        }

        public void SetWindowPosition(double x, double y)
        {
            Left = x;
            Top = y;
        }

        public void SavePosition()
        {
            SettingsManager.SaveOverlayPosition(Left, Top);
        }

        public void LoadPosition()
        {
            var position = SettingsManager.LoadOverlayPosition();
            if (position.HasValue)
            {
                SetWindowPosition(position.Value.X, position.Value.Y);
            }
            else
            {
                // Posición por defecto (esquina superior derecha)
                SetWindowPosition(SystemParameters.PrimaryScreenWidth - 200, 50);
            }
        }

        protected override void OnLocationChanged(EventArgs e)
        {
            base.OnLocationChanged(e);
            // Guardar posición cuando se mueve la ventana
            if (IsVisible)
            {
                SavePosition();
            }
        }

        protected override void OnClosed(EventArgs e)
        {
            SavePosition();
            _checkTimer?.Stop();
            base.OnClosed(e);
        }

        private void ApplyLayoutMode()
        {
            // Ajustar plantilla y paddings según modo compacto
            var templateKey = _isCompact ? "CompactItemTemplate" : "FullItemTemplate";
            CharactersList.ItemTemplate = (DataTemplate)FindResource(templateKey);

            if (OverlayContainer != null)
            {
                OverlayContainer.Padding = _isCompact ? new Thickness(3) : new Thickness(4);
            }

            // Limpiar caché porque cambiaron los contenedores
            _borderCache.Clear();
            _lastActiveProcessId = null;

            UpdateCompactToggleVisuals();
        }

        public void SetCompactMode(bool compact)
        {
            if (_isCompact == compact) return;

            _isCompact = compact;
            ApplyLayoutMode();
            HighlightCurrentCharacter();
            OnCompactChanged?.Invoke(_isCompact);
        }

        private void UpdateCompactToggleVisuals()
        {
            if (CompactToggleText == null || CompactToggleButton == null) return;

            // Flecha indica la acción disponible
            CompactToggleText.Text = _isCompact ? "→" : "←";
            CompactToggleButton.ToolTip = _isCompact ? "Mostrar nombres" : "Mostrar solo iconos";
        }
    }
}

