using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using DofusTabs.Core;
using DofusTabs.Utils;

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

        public MainWindow()
        {
            InitializeComponent();
            _windowManager = new WindowManager();
            InitializeHotkeys();
            LoadSettings();
            RefreshWindowsList();
            UpdateHotkeyDisplay();
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
            }
        }

        private void InitializeHotkeys()
        {
            _hotkeyManager = new HotkeyManager(this, _windowManager);
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

            WindowsDataGrid.ItemsSource = windows;
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
                _currentCapturingWindow.IndividualHotkey = display;
                textBox.Text = display;

                // Registrar el atajo individual para esta ventana
                RegisterIndividualHotkey(_currentCapturingWindow, modifiers, key);
                
                StatusTextBlock.Text = $"Atajo asignado a {_currentCapturingWindow.CharacterName}: {display}";
                
                _isCapturingHotkey = false;
                _currentCapturingWindow = null;
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
            }
        }

        private void HotkeyTextBox_LostFocus(object sender, RoutedEventArgs e)
        {
            _isCapturingHotkey = false;
        }

        private void NextHotkeyTextBox_KeyDown(object sender, KeyEventArgs e)
        {
            if (!_isCapturingHotkey) return;

            e.Handled = true;

            if (e.Key == Key.Tab || e.Key == Key.Enter || e.Key == Key.Escape)
            {
                UpdateHotkeyDisplay();
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
                }
                catch
                {
                    StatusTextBlock.Text = "Error al registrar el atajo";
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
                }
                catch
                {
                    StatusTextBlock.Text = "Error al registrar el atajo";
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
        }

        private void ClearPreviousHotkeyButton_Click(object sender, RoutedEventArgs e)
        {
            PreviousHotkeyTextBox.Text = "Ninguno";
            StatusTextBlock.Text = "Atajo eliminado. Configura uno nuevo haciendo clic en el campo.";
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
            base.OnClosed(e);
        }
    }
}

