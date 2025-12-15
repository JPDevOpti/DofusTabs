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
        private static readonly SolidColorBrush ActiveBrush = new SolidColorBrush(Color.FromArgb(200, 0, 120, 212));
        private static readonly SolidColorBrush ActiveBorderBrush = new SolidColorBrush(Color.FromArgb(255, 0, 90, 158));

        public OverlayWindow()
        {
            InitializeComponent();
            _windowManager = new WindowManager();
            
            // Timer solo para actualizar el resaltado
            _checkTimer = new DispatcherTimer();
            _checkTimer.Interval = TimeSpan.FromMilliseconds(200);
            _checkTimer.Tick += Timer_Tick;
            
            Loaded += OverlayWindow_Loaded;
        }

        private void OverlayWindow_Loaded(object sender, RoutedEventArgs e)
        {
            UpdateCharactersList();
            _checkTimer.Start();
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ButtonState == MouseButtonState.Pressed)
            {
                this.DragMove();
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
            if (_currentActiveWindow == null) return;

            // Actualizar resaltado de forma optimizada
            foreach (var item in CharactersList.Items)
            {
                if (item is WindowInfo windowInfo)
                {
                    var container = CharactersList.ItemContainerGenerator.ContainerFromItem(item) as FrameworkElement;
                    if (container != null)
                    {
                        var border = FindVisualChild<Border>(container);
                        if (border != null)
                        {
                            if (windowInfo.ProcessId == _currentActiveWindow.ProcessId)
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
    }
}

