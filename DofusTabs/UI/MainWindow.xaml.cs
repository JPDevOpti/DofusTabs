using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using DofusTabs.Core;
using DofusTabs.Utils;
using System.ComponentModel;
using System.Threading.Tasks;
using Forms = System.Windows.Forms;
using Drawing = System.Drawing;

namespace DofusTabs.UI
{
    public partial class MainWindow : Window
    {
        private WindowManager _windowManager;
        private HotkeyManager? _hotkeyManager;
        private ModifierKeys _capturedModifiers;
        private Key _capturedKey;
        private bool _isCapturingHotkey = false;
        private WindowInfo? _currentCapturingWindow = null;
        private WindowInfo? _draggedWindow = null;
        private System.Windows.Point _dragStartPoint;
        private OverlayWindow? _overlayWindow;
        private Forms.NotifyIcon? _notifyIcon;
        private bool _isExiting = false;

        public MainWindow()
        {
            InitializeComponent();
            _windowManager = new WindowManager();
            InitializeHotkeys();
            InitializeOverlay();
            SetupTrayIcon();
            
            // Cargar configuración después de que la ventana esté cargada
            Loaded += MainWindow_Loaded;
            StateChanged += MainWindow_StateChanged;
        }

        private void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            LoadSettings();
            RefreshWindowsList();
            if (_overlayWindow != null && _overlayWindow.IsVisible)
            {
                RefreshOverlayListFromGrid();
            }
            UpdateHotkeyDisplay();
            
            // Re-registrar todos los atajos después de cargar la configuración
            if (_hotkeyManager != null)
            {
                _hotkeyManager.ReRegisterHotkeys();
            }

            // Asegurar que el icono de bandeja quede visible
            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = true;
            }
        }

        private void InitializeOverlay()
        {
            _overlayWindow = new OverlayWindow();
            _overlayWindow.OnOverlayHidden += OverlayWindow_OnOverlayHidden;
            _overlayWindow.OnCompactChanged += OverlayWindow_OnCompactChanged;
            _overlayWindow.LoadPosition();
            _overlayWindow.Hide();
        }

        private void SetupTrayIcon()
        {
            _notifyIcon = new Forms.NotifyIcon();
            try
            {
                // En publish single-file, Assembly.Location puede venir vacío.
                var exePath = Environment.ProcessPath ?? System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
                if (!string.IsNullOrWhiteSpace(exePath))
                {
                    _notifyIcon.Icon = Drawing.Icon.ExtractAssociatedIcon(exePath);
                }
            }
            catch
            {
                // Si falla, no asignar icono
            }

            _notifyIcon.Text = "DofusTabs (en ejecución)";
            _notifyIcon.Visible = true;
            _notifyIcon.DoubleClick += (s, e) => RestoreFromTray();

            var menu = new Forms.ContextMenuStrip();
            menu.Items.Add("Mostrar", null, (s, e) => RestoreFromTray());
            menu.Items.Add("Salir", null, (s, e) => ExitFromTray());
            _notifyIcon.ContextMenuStrip = menu;
        }

        private void RestoreFromTray()
        {
            Show();
            ShowInTaskbar = true;
            WindowState = WindowState.Normal;
            Activate();
        }

        private void ExitFromTray()
        {
            _isExiting = true;
            Close();
        }

        protected override void OnClosing(CancelEventArgs e)
        {
            SaveOverlayState();
            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = false;
                _notifyIcon.Dispose();
            }
            base.OnClosing(e);
        }

        private void MainWindow_StateChanged(object? sender, EventArgs e)
        {
            if (WindowState == WindowState.Minimized && !_isExiting)
            {
                Hide();
                ShowInTaskbar = false;
                if (_notifyIcon != null)
                {
                    _notifyIcon.Visible = true;
                }
            }
            else if (WindowState == WindowState.Normal)
            {
                ShowInTaskbar = true;
            }
        }

        private void ToggleOverlayButton_Click(object sender, RoutedEventArgs e)
        {
            if (_overlayWindow == null)
            {
                InitializeOverlay();
            }

            if (_overlayWindow != null)
            {
                if (_overlayWindow.IsVisible)
                {
                    HideOverlay();
                }
                else
                {
                    ShowOverlay();
                }
            }
        }

        private void OverlayWindow_OnOverlayHidden()
        {
            ToggleOverlayButton.Content = "Mostrar Overlay";
            SaveOverlayState();
        }

        private void OverlayWindow_OnCompactChanged(bool isCompact)
        {
            SaveOverlayState();
        }

        private void LoadSettings()
        {
            var settings = SettingsManager.LoadSettings();
            if (settings != null && _hotkeyManager != null)
            {
                // Cargar atajos globales
                if (!string.IsNullOrEmpty(settings.NextHotkeyKey))
                {
                    var modifiers = SettingsManager.ParseModifiers(settings.NextHotkeyModifiers);
                    if (Enum.TryParse<Key>(settings.NextHotkeyKey, out var key))
                    {
                        _hotkeyManager.UpdateNextHotkey(modifiers, key);
                    }
                }

                if (!string.IsNullOrEmpty(settings.PreviousHotkeyKey))
                {
                    var modifiers = SettingsManager.ParseModifiers(settings.PreviousHotkeyModifiers);
                    if (Enum.TryParse<Key>(settings.PreviousHotkeyKey, out var key))
                    {
                        _hotkeyManager.UpdatePreviousHotkey(modifiers, key);
                    }
                }

                ApplyOverlaySettings(settings);
            }
        }

        private void ApplyOverlaySettings(SettingsManager.AppSettings settings)
        {
            if (_overlayWindow == null)
            {
                InitializeOverlay();
            }

            if (_overlayWindow == null)
            {
                return;
            }

            _overlayWindow.SetCompactMode(settings.OverlayCompact);

            if (settings.OverlayVisible)
            {
                ShowOverlay();
            }
            else
            {
                ToggleOverlayButton.Content = "Mostrar Overlay";
            }
        }

        private void ShowOverlay()
        {
            if (_overlayWindow == null)
            {
                InitializeOverlay();
            }

            if (_overlayWindow == null)
            {
                return;
            }

            _overlayWindow.Show();
            _overlayWindow.Topmost = true;
            RefreshOverlayListFromGrid();
            ToggleOverlayButton.Content = "Ocultar Overlay";
            SaveOverlayState();
        }

        private void HideOverlay()
        {
            if (_overlayWindow == null) return;

            _overlayWindow.Hide();
            ToggleOverlayButton.Content = "Mostrar Overlay";
            SaveOverlayState();
        }

        private void RefreshOverlayListFromGrid()
        {
            if (_overlayWindow == null) return;

            var currentWindows = WindowsDataGrid.ItemsSource as System.Collections.IEnumerable;
            if (currentWindows != null)
            {
                var windowsList = currentWindows.Cast<WindowInfo>().ToList();
                _overlayWindow.RefreshWindowsList(windowsList);
            }
            else
            {
                _overlayWindow.RefreshWindowsList();
            }
        }

        private void SaveOverlayState()
        {
            if (_overlayWindow == null) return;

            SettingsManager.SaveOverlayState(
                _overlayWindow.IsVisible,
                _overlayWindow.IsCompact,
                _overlayWindow.Left,
                _overlayWindow.Top);
        }


        private void InitializeHotkeys()
        {
            _hotkeyManager = new HotkeyManager(this);
            _hotkeyManager.OnNextWindow += () => Dispatcher.Invoke(() => SwitchToNextWindow());
            _hotkeyManager.OnPreviousWindow += () => Dispatcher.Invoke(() => SwitchToPreviousWindow());
            _hotkeyManager.OnIndividualHotkey += (windowInfo) => Dispatcher.Invoke(() => SwitchToIndividualWindow(windowInfo));
        }

        private void SwitchToIndividualWindow(WindowInfo windowInfo)
        {
            if (windowInfo.IsEnabled)
            {
                // Buscar la ventana por ProcessId para asegurar que encontramos la correcta
                var windows = _windowManager.GetDofusWindows();
                var targetWindow = windows.FirstOrDefault(w => w.ProcessId == windowInfo.ProcessId);
                
                if (targetWindow != null)
                {
                    int index = windows.IndexOf(targetWindow);
                    if (index >= 0)
                    {
                        _windowManager.SwitchToWindow(targetWindow, index);
                        StatusTextBlock.Text = $"Cambiado a {targetWindow.CharacterName}";
                    }
                }
            }
        }

        private void UpdateHotkeyDisplay()
        {
            if (_hotkeyManager != null)
            {
                NextHotkeyTextBox.Text = _hotkeyManager.GetNextHotkeyDisplay();
                PreviousHotkeyTextBox.Text = _hotkeyManager.GetPreviousHotkeyDisplay();
            }
        }

        private void RefreshWindowsList()
        {
            // Guardar el estado de los checkboxes y atajos antes de actualizar
            var previousWindows = WindowsDataGrid.ItemsSource as System.Collections.IEnumerable;
            var enabledStates = new Dictionary<uint, bool>();
            var hotkeyStates = new Dictionary<uint, string>();
            var orderStates = new Dictionary<uint, int>();
            
            if (previousWindows != null)
            {
                foreach (WindowInfo win in previousWindows)
                {
                    enabledStates[win.ProcessId] = win.IsEnabled;
                    hotkeyStates[win.ProcessId] = win.IndividualHotkey;
                    orderStates[win.ProcessId] = win.DisplayOrder;
                }
            }
            else
            {
                // Cargar desde archivo si es la primera vez
                var settings = SettingsManager.LoadSettings();
                if (settings != null)
                {
                    foreach (var ws in settings.Windows)
                    {
                        enabledStates[ws.ProcessId] = ws.IsEnabled;
                        hotkeyStates[ws.ProcessId] = ws.IndividualHotkey;
                        orderStates[ws.ProcessId] = ws.DisplayOrder;
                    }
                }
            }

            var windows = _windowManager.GetDofusWindows();
            
            // Restaurar el estado de los checkboxes, atajos y orden
            foreach (var window in windows)
            {
                if (enabledStates.ContainsKey(window.ProcessId))
                {
                    window.IsEnabled = enabledStates[window.ProcessId];
                }
                if (hotkeyStates.ContainsKey(window.ProcessId))
                {
                    window.IndividualHotkey = hotkeyStates[window.ProcessId];
                    // Re-registrar el atajo individual si existe
                    if (!string.IsNullOrEmpty(window.IndividualHotkey) && _hotkeyManager != null)
                    {
                        var hotkeyParts = ParseHotkeyString(window.IndividualHotkey);
                        if (hotkeyParts.modifiers != ModifierKeys.None && hotkeyParts.key != Key.None)
                        {
                            _hotkeyManager.RegisterIndividualHotkey(window, hotkeyParts.modifiers, hotkeyParts.key);
                        }
                    }
                }
                if (orderStates.ContainsKey(window.ProcessId))
                {
                    window.DisplayOrder = orderStates[window.ProcessId];
                }
            }

            // Ordenar explícitamente por DisplayOrder para asegurar el mismo orden que el overlay
            var sortedWindows = windows.OrderBy(w => w.DisplayOrder).ToList();
            WindowsDataGrid.ItemsSource = sortedWindows;
            var enabledCount = windows.Count(w => w.IsEnabled);
            WindowsCountTextBlock.Text = $"{windows.Count} ventanas detectadas ({enabledCount} habilitadas)";

            if (windows.Count == 0)
            {
                StatusTextBlock.Text = "No se encontraron ventanas de Dofus";
            }
            else
            {
                StatusTextBlock.Text = "Listo";
            }

            // Actualizar overlay con la misma lista ordenada
            if (_overlayWindow != null && _overlayWindow.IsVisible)
            {
                _overlayWindow.RefreshWindowsList(sortedWindows);
            }

            // Guardar configuración
            SaveSettings();
        }

        private (ModifierKeys modifiers, Key key) ParseHotkeyString(string hotkeyString)
        {
            if (string.IsNullOrEmpty(hotkeyString) || hotkeyString == "Ninguno")
                return (ModifierKeys.None, Key.None);

            var parts = hotkeyString.Split(new[] { " + " }, StringSplitOptions.RemoveEmptyEntries);
            var modifiers = ModifierKeys.None;
            Key key = Key.None;

            for (int i = 0; i < parts.Length - 1; i++)
            {
                var part = parts[i].Trim();
                if (part == "Ctrl" || part == "Control")
                    modifiers |= ModifierKeys.Control;
                else if (part == "Alt")
                    modifiers |= ModifierKeys.Alt;
                else if (part == "Shift")
                    modifiers |= ModifierKeys.Shift;
            }

            if (parts.Length > 0)
            {
                var keyString = parts[parts.Length - 1].Trim();
                if (Enum.TryParse<Key>(keyString, out var parsedKey))
                {
                    key = parsedKey;
                }
            }

            return (modifiers, key);
        }

        private void SaveSettings()
        {
            var windows = WindowsDataGrid.ItemsSource as System.Collections.IEnumerable;
            if (windows != null)
            {
                var windowsList = windows.Cast<WindowInfo>().ToList();
                SettingsManager.SaveSettings(windowsList, _hotkeyManager);
            }
        }

        private void RefreshButton_Click(object sender, RoutedEventArgs e)
        {
            RefreshWindowsList();
        }

        private void SwitchToNextWindow()
        {
            // Solo permitir si hay una ventana de Dofus activa
            var currentActive = _windowManager.GetCurrentActiveWindow();
            if (currentActive == null)
            {
                StatusTextBlock.Text = "Enfoca una ventana de Dofus primero";
                return;
            }

            if (_windowManager.SwitchToNextWindow())
            {
                StatusTextBlock.Text = "Cambiado a siguiente ventana";
                RefreshWindowsList();
            }
            else
            {
                StatusTextBlock.Text = "No se pudo cambiar de ventana";
            }
        }

        private void SwitchToPreviousWindow()
        {
            // Solo permitir si hay una ventana de Dofus activa
            var currentActive = _windowManager.GetCurrentActiveWindow();
            if (currentActive == null)
            {
                StatusTextBlock.Text = "Enfoca una ventana de Dofus primero";
                return;
            }

            if (_windowManager.SwitchToPreviousWindow())
            {
                StatusTextBlock.Text = "Cambiado a ventana anterior";
                RefreshWindowsList();
            }
            else
            {
                StatusTextBlock.Text = "No se pudo cambiar de ventana";
            }
        }

        private void WindowsDataGrid_SelectionChanged(object sender, System.Windows.Controls.SelectionChangedEventArgs e)
        {
            if (WindowsDataGrid.SelectedItem is WindowInfo windowInfo)
            {
                StatusTextBlock.Text = $"Seleccionada: {windowInfo.DisplayTitle}";
            }
        }

        private void CheckBox_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.CheckBox checkBox && checkBox.DataContext is WindowInfo windowInfo)
            {
                windowInfo.IsEnabled = checkBox.IsChecked ?? true;
                var enabledCount = _windowManager.GetDofusWindows().Count(w => w.IsEnabled);
                StatusTextBlock.Text = $"{enabledCount} ventana(s) habilitada(s)";
                
                // Actualizar overlay si está visible con la lista actualizada
                if (_overlayWindow != null && _overlayWindow.IsVisible)
                {
                    var currentWindows = WindowsDataGrid.ItemsSource as System.Collections.IEnumerable;
                    if (currentWindows != null)
                    {
                        var windowsList = currentWindows.Cast<WindowInfo>().ToList();
                        _overlayWindow.RefreshWindowsList(windowsList);
                    }
                }
                
                // Guardar configuración
                SaveSettings();
            }
        }

        private void IndividualHotkeyTextBox_GotFocus(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.TextBox textBox)
            {
                // Obtener el WindowInfo del DataContext
                var row = FindParent<System.Windows.Controls.DataGridRow>(textBox);
                if (row != null && row.Item is WindowInfo windowInfo)
                {
                    _currentCapturingWindow = windowInfo;
                    textBox.Text = "Presiona la combinación de teclas...";
                    _isCapturingHotkey = true;
                    _hotkeyManager?.SetHotkeyActionsSuspended(true);
                }
            }
        }

        private static T? FindParent<T>(System.Windows.DependencyObject child) where T : System.Windows.DependencyObject
        {
            System.Windows.DependencyObject parentObject = System.Windows.Media.VisualTreeHelper.GetParent(child);
            if (parentObject == null) return null;
            if (parentObject is T parent) return parent;
            return FindParent<T>(parentObject);
        }

        private void IndividualHotkeyTextBox_LostFocus(object sender, RoutedEventArgs e)
        {
            _isCapturingHotkey = false;
            _currentCapturingWindow = null;
            _hotkeyManager?.SetHotkeyActionsSuspended(false);
        }

        private void IndividualHotkeyTextBox_KeyDown(object sender, KeyEventArgs e)
        {
            if (!_isCapturingHotkey || _currentCapturingWindow == null) return;

            if (sender is System.Windows.Controls.TextBox textBox)
            {
                e.Handled = true;

                if (e.Key == Key.Tab || e.Key == Key.Enter || e.Key == Key.Escape)
                {
                    if (string.IsNullOrEmpty(_currentCapturingWindow.IndividualHotkey))
                    {
                        textBox.Text = "Ninguno";
                    }
                    else
                    {
                        textBox.Text = _currentCapturingWindow.IndividualHotkey;
                    }
                    _isCapturingHotkey = false;
                    _currentCapturingWindow = null;
                    _hotkeyManager?.SetHotkeyActionsSuspended(false);
                    textBox.MoveFocus(new System.Windows.Input.TraversalRequest(System.Windows.Input.FocusNavigationDirection.Next));
                    return;
                }

                ModifierKeys modifiers = Keyboard.Modifiers;
                Key key = e.Key;

                if (key == Key.LeftCtrl || key == Key.RightCtrl || 
                    key == Key.LeftAlt || key == Key.RightAlt || 
                    key == Key.LeftShift || key == Key.RightShift)
                {
                    return;
                }

                string display = FormatHotkey(modifiers, key);
                
                // Verificar si el atajo ya está en uso por otra ventana
                var windows = WindowsDataGrid.ItemsSource as System.Collections.IEnumerable;
                if (windows != null)
                {
                    var windowsList = windows.Cast<WindowInfo>().ToList();
                    var conflictWindow = windowsList.FirstOrDefault(w => 
                        w.ProcessId != _currentCapturingWindow.ProcessId && 
                        w.IndividualHotkey == display);
                    
                    if (conflictWindow != null)
                    {
                        // Liberar el atajo de la ventana anterior
                        if (_hotkeyManager != null)
                        {
                            _hotkeyManager.UnregisterIndividualHotkey(conflictWindow.ProcessId);
                        }
                        conflictWindow.IndividualHotkey = string.Empty;
                        StatusTextBlock.Text = $"Atajo removido de {conflictWindow.CharacterName} y asignado a {_currentCapturingWindow.CharacterName}";
                    }
                    else
                    {
                        StatusTextBlock.Text = $"Atajo asignado a {_currentCapturingWindow.CharacterName}: {display}";
                    }
                }
                
                _currentCapturingWindow.IndividualHotkey = display;
                textBox.Text = display;

                // Registrar el atajo individual para esta ventana
                RegisterIndividualHotkey(_currentCapturingWindow, modifiers, key);
                
                _isCapturingHotkey = false;
                _currentCapturingWindow = null;
                _hotkeyManager?.SetHotkeyActionsSuspended(false);
            }
        }

        private void RegisterIndividualHotkey(WindowInfo windowInfo, ModifierKeys modifiers, Key key)
        {
            if (_hotkeyManager != null)
            {
                _hotkeyManager.RegisterIndividualHotkey(windowInfo, modifiers, key);
                SaveSettings();
            }
        }

        private void WindowsDataGrid_PreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            var source = e.OriginalSource as System.Windows.DependencyObject;
            if (source != null)
            {
                var row = FindParent<System.Windows.Controls.DataGridRow>(source);
                if (row != null && row.Item is WindowInfo windowInfo)
                {
                    _draggedWindow = windowInfo;
                    _dragStartPoint = e.GetPosition(WindowsDataGrid);
                }
            }
        }

        private void WindowsDataGrid_PreviewMouseMove(object sender, MouseEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed && _draggedWindow != null)
            {
                var currentPoint = e.GetPosition(WindowsDataGrid);
                if (Math.Abs(currentPoint.X - _dragStartPoint.X) > 5 || Math.Abs(currentPoint.Y - _dragStartPoint.Y) > 5)
                {
                    WindowsDataGrid.AllowDrop = true;
                    DragDrop.DoDragDrop(WindowsDataGrid, _draggedWindow, DragDropEffects.Move);
                    _draggedWindow = null;
                }
            }
        }

        private void WindowsDataGrid_DragOver(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(typeof(WindowInfo)))
            {
                e.Effects = DragDropEffects.Move;
                e.Handled = true;
            }
        }

        private void WindowsDataGrid_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(typeof(WindowInfo)) && _draggedWindow != null)
            {
                var source = e.OriginalSource as System.Windows.DependencyObject;
                if (source != null)
                {
                    var targetRow = FindParent<System.Windows.Controls.DataGridRow>(source);
                    if (targetRow != null && targetRow.Item is WindowInfo targetWindow && targetWindow != _draggedWindow)
                    {
                        var windows = WindowsDataGrid.ItemsSource as System.Collections.IEnumerable;
                        if (windows != null)
                        {
                            var windowsList = windows.Cast<WindowInfo>().OrderBy(w => w.DisplayOrder).ToList();
                            int draggedIndex = windowsList.IndexOf(_draggedWindow);
                            int targetIndex = windowsList.IndexOf(targetWindow);

                            if (draggedIndex >= 0 && targetIndex >= 0)
                            {
                                // Mover el elemento arrastrado a la nueva posición
                                windowsList.RemoveAt(draggedIndex);
                                windowsList.Insert(targetIndex, _draggedWindow);

                                // Reasignar DisplayOrder secuencialmente
                                for (int i = 0; i < windowsList.Count; i++)
                                {
                                    windowsList[i].DisplayOrder = i;
                                }

                                // Refrescar la lista manteniendo el orden
                                RefreshWindowsList();

                                // Sincronizar overlay con el nuevo orden
                                if (_overlayWindow != null && _overlayWindow.IsVisible)
                                {
                                    _overlayWindow.RefreshWindowsList(windowsList);
                                }
                                
                                StatusTextBlock.Text = $"Orden actualizado: {_draggedWindow.CharacterName} movido a posición {targetIndex + 1}";
                            }
                        }
                    }
                }
                _draggedWindow = null;
            }
        }

        private void HotkeyTextBox_GotFocus(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.TextBox textBox)
            {
                textBox.Text = "Presiona la combinación de teclas...";
                _isCapturingHotkey = true;
                _hotkeyManager?.SetHotkeyActionsSuspended(true);
            }
        }

        private void HotkeyTextBox_LostFocus(object sender, RoutedEventArgs e)
        {
            _isCapturingHotkey = false;
            _hotkeyManager?.SetHotkeyActionsSuspended(false);
        }

        private void NextHotkeyTextBox_KeyDown(object sender, KeyEventArgs e)
        {
            if (!_isCapturingHotkey) return;

            e.Handled = true;

            if (e.Key == Key.Tab || e.Key == Key.Enter || e.Key == Key.Escape)
            {
                UpdateHotkeyDisplay();
                _hotkeyManager?.SetHotkeyActionsSuspended(false);
                NextHotkeyTextBox.MoveFocus(new System.Windows.Input.TraversalRequest(System.Windows.Input.FocusNavigationDirection.Next));
                return;
            }

            ModifierKeys modifiers = Keyboard.Modifiers;
            Key key = e.Key;

            if (key == Key.LeftCtrl || key == Key.RightCtrl || 
                key == Key.LeftAlt || key == Key.RightAlt || 
                key == Key.LeftShift || key == Key.RightShift)
            {
                return;
            }

            _capturedModifiers = modifiers;
            _capturedKey = key;

            string display = FormatHotkey(modifiers, key);
            NextHotkeyTextBox.Text = display;

            if (_hotkeyManager != null)
            {
                try
                {
                    _hotkeyManager.UpdateNextHotkey(modifiers, key);
                    StatusTextBlock.Text = "Atajo actualizado: " + display;
                    SaveSettings();
                    _hotkeyManager.SetHotkeyActionsSuspended(false);
                }
                catch
                {
                    StatusTextBlock.Text = "Error al registrar el atajo";
                    _hotkeyManager.SetHotkeyActionsSuspended(false);
                }
            }
        }

        private void PreviousHotkeyTextBox_KeyDown(object sender, KeyEventArgs e)
        {
            if (!_isCapturingHotkey) return;

            e.Handled = true;

            if (e.Key == Key.Tab || e.Key == Key.Enter || e.Key == Key.Escape)
            {
                UpdateHotkeyDisplay();
                _hotkeyManager?.SetHotkeyActionsSuspended(false);
                PreviousHotkeyTextBox.MoveFocus(new System.Windows.Input.TraversalRequest(System.Windows.Input.FocusNavigationDirection.Next));
                return;
            }

            ModifierKeys modifiers = Keyboard.Modifiers;
            Key key = e.Key;

            if (key == Key.LeftCtrl || key == Key.RightCtrl || 
                key == Key.LeftAlt || key == Key.RightAlt || 
                key == Key.LeftShift || key == Key.RightShift)
            {
                return;
            }

            _capturedModifiers = modifiers;
            _capturedKey = key;

            string display = FormatHotkey(modifiers, key);
            PreviousHotkeyTextBox.Text = display;

            if (_hotkeyManager != null)
            {
                try
                {
                    _hotkeyManager.UpdatePreviousHotkey(modifiers, key);
                    StatusTextBlock.Text = "Atajo actualizado: " + display;
                    SaveSettings();
                    _hotkeyManager.SetHotkeyActionsSuspended(false);
                }
                catch
                {
                    StatusTextBlock.Text = "Error al registrar el atajo";
                    _hotkeyManager.SetHotkeyActionsSuspended(false);
                }
            }
        }


        private string FormatHotkey(ModifierKeys modifiers, Key key)
        {
            string result = "";
            if ((modifiers & ModifierKeys.Control) != 0) result += "Ctrl + ";
            if ((modifiers & ModifierKeys.Alt) != 0) result += "Alt + ";
            if ((modifiers & ModifierKeys.Shift) != 0) result += "Shift + ";
            result += key.ToString();
            return result;
        }

        private void ClearNextHotkeyButton_Click(object sender, RoutedEventArgs e)
        {
            NextHotkeyTextBox.Text = "Ninguno";
            StatusTextBlock.Text = "Atajo eliminado. Configura uno nuevo haciendo clic en el campo.";
            SaveSettings();
        }

        private void ClearPreviousHotkeyButton_Click(object sender, RoutedEventArgs e)
        {
            PreviousHotkeyTextBox.Text = "Ninguno";
            StatusTextBlock.Text = "Atajo eliminado. Configura uno nuevo haciendo clic en el campo.";
            SaveSettings();
        }


        private void ClearIndividualHotkeyButton_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button button && button.Tag is WindowInfo windowInfo)
            {
                // Desregistrar el atajo
                if (_hotkeyManager != null)
                {
                    _hotkeyManager.UnregisterIndividualHotkey(windowInfo.ProcessId);
                }

                // Limpiar el atajo
                windowInfo.IndividualHotkey = string.Empty;
                
                // Actualizar la interfaz
                RefreshWindowsList();
                StatusTextBlock.Text = $"Atajo eliminado para {windowInfo.CharacterName}";
                
                // Guardar configuración
                SaveSettings();
            }
        }

        private void ExitMenuItem_Click(object sender, RoutedEventArgs e)
        {
            Application.Current.Shutdown();
        }

        private void AboutMenuItem_Click(object sender, RoutedEventArgs e)
        {
            MessageBox.Show("DofusTabs v1.0\n\n" +
                          "Aplicación para controlar y navegar entre ventanas de Dofus.\n\n" +
                          "Usa los atajos de teclado para cambiar rápidamente entre ventanas.",
                          "Acerca de",
                          MessageBoxButton.OK,
                          MessageBoxImage.Information);
        }

        protected override void OnClosed(EventArgs e)
        {
            SaveSettings();
            _hotkeyManager?.Dispose();
            _overlayWindow?.Close();
            base.OnClosed(e);
        }
    }
}

