using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using DofusTabs.Core;
using Drawing = System.Drawing;
using Forms = System.Windows.Forms;

namespace DofusTabs.UI
{
    public partial class MainWindow : Window
    {
        private readonly WindowManager _windowManager;
        private readonly WindowEmbeddingService _windowEmbeddingService;
        private List<WindowInfo> _detectedWindows = new List<WindowInfo>();
        private List<WindowInfo> _sidebarWindows = new List<WindowInfo>();
        private uint? _activeProcessId;
        private Forms.NotifyIcon? _notifyIcon;
        private bool _isExiting;
        private bool _exitConfirmed;

        public MainWindow()
        {
            InitializeComponent();

            _windowManager = new WindowManager();
            _windowEmbeddingService = new WindowEmbeddingService();

            SetupTrayIcon();

            Loaded += MainWindow_Loaded;
            StateChanged += MainWindow_StateChanged;
        }

        private void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            GameHostPanel.Resize += (_, _) => ResizeEmbeddedClient();
            RefreshDetectedWindows(selectFallbackAccount: true);
        }

        private void SetupTrayIcon()
        {
            _notifyIcon = new Forms.NotifyIcon();
            try
            {
                var exePath = Environment.ProcessPath ?? System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
                if (!string.IsNullOrWhiteSpace(exePath))
                {
                    _notifyIcon.Icon = Drawing.Icon.ExtractAssociatedIcon(exePath);
                }
            }
            catch
            {
                // Si no se puede extraer icono, se mantiene el icono por defecto.
            }

            _notifyIcon.Text = "DofusMaster (en ejecución)";
            _notifyIcon.Visible = true;
            _notifyIcon.DoubleClick += (_, _) => RestoreFromTray();

            var menu = new Forms.ContextMenuStrip();
            menu.Items.Add("Mostrar", null, (_, _) => RestoreFromTray());
            menu.Items.Add("Salir", null, (_, _) => ExitFromTray());
            _notifyIcon.ContextMenuStrip = menu;
        }

        private void RefreshButton_Click(object sender, RoutedEventArgs e)
        {
            RefreshDetectedWindows(selectFallbackAccount: true);
        }

        private void RefreshDetectedWindows(bool selectFallbackAccount)
        {
            _detectedWindows = _windowManager
                .GetDofusWindows()
                .OrderBy(w => w.DisplayOrder)
                .ThenBy(w => w.CharacterName)
                .ToList();

            if (_activeProcessId.HasValue && !_detectedWindows.Any(w => w.ProcessId == _activeProcessId.Value))
            {
                _activeProcessId = null;
                _windowEmbeddingService.RestoreEmbeddedWindow();
            }

            UpdateActiveFlags();
            UpdateSidebarBubbleSource();

            if (!_detectedWindows.Any())
            {
                HostTitleText.Text = "Cliente Activo";
                StatusTextBlock.Text = "No se detectaron clientes de Dofus";
                EmptyHostText.Visibility = Visibility.Visible;
                return;
            }

            if (selectFallbackAccount && !_activeProcessId.HasValue)
            {
                ActivateAccount(_detectedWindows[0]);
            }
            else if (_activeProcessId.HasValue)
            {
                var activeWindow = _detectedWindows.FirstOrDefault(w => w.ProcessId == _activeProcessId.Value);
                if (activeWindow != null)
                {
                    HostTitleText.Text = $"{activeWindow.CharacterName} - {activeWindow.CharacterClass}";
                    StatusTextBlock.Text = $"Activo: {activeWindow.CharacterName}";
                    EmptyHostText.Visibility = Visibility.Collapsed;
                }
            }
        }

        private void AccountBubble_Click(object sender, RoutedEventArgs e)
        {
            if (sender is Button button && button.Tag is WindowInfo windowInfo)
            {
                ActivateAccount(windowInfo);
            }
        }

        private void ActivateAccount(WindowInfo windowInfo)
        {
            if (windowInfo.Handle == IntPtr.Zero)
            {
                StatusTextBlock.Text = "No se puede activar la cuenta seleccionada";
                return;
            }

            bool embedded = _windowEmbeddingService.TryEmbed(
                windowInfo,
                GameHostPanel.Handle,
                Math.Max(1, GameHostPanel.ClientSize.Width),
                Math.Max(1, GameHostPanel.ClientSize.Height));

            if (!embedded)
            {
                StatusTextBlock.Text = "No se pudo embeber la ventana seleccionada";
                return;
            }

            _activeProcessId = windowInfo.ProcessId;
            UpdateActiveFlags();
            UpdateSidebarBubbleSource();

            HostTitleText.Text = $"{windowInfo.CharacterName} - {windowInfo.CharacterClass}";
            StatusTextBlock.Text = $"Cuenta activa: {windowInfo.CharacterName}";
            EmptyHostText.Visibility = Visibility.Collapsed;
        }

        private void RestoreEmbeddedClientButton_Click(object sender, RoutedEventArgs e)
        {
            _windowEmbeddingService.RestoreEmbeddedWindow();
            _activeProcessId = null;
            UpdateActiveFlags();
            UpdateSidebarBubbleSource();

            HostTitleText.Text = "Cliente Activo";
            StatusTextBlock.Text = "Cliente restaurado a su ventana original";
            EmptyHostText.Visibility = Visibility.Visible;
        }

        private void SettingsButton_Click(object sender, RoutedEventArgs e)
        {
            MessageBox.Show(
                "Panel de configuración en construcción.\n\n" +
                "Próximo paso: hotkeys, temas y grupos de cuentas.",
                "Ajustes",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
        }

        private void ResizeEmbeddedClient()
        {
            _windowEmbeddingService.ResizeEmbeddedWindow(
                Math.Max(1, GameHostPanel.ClientSize.Width),
                Math.Max(1, GameHostPanel.ClientSize.Height));
        }

        private void UpdateCounters()
        {
            AccountsSummaryText.Text = $"{_detectedWindows.Count} cuentas";
        }

        private void UpdateActiveFlags()
        {
            foreach (var window in _detectedWindows)
            {
                window.IsActive = _activeProcessId.HasValue && window.ProcessId == _activeProcessId.Value;
            }

            UpdateCounters();
        }

        private void UpdateSidebarBubbleSource()
        {
            WindowInfo? sidebarWindow = null;

            if (_activeProcessId.HasValue)
            {
                sidebarWindow = _detectedWindows.FirstOrDefault(w => w.ProcessId == _activeProcessId.Value);
            }

            if (sidebarWindow == null && _detectedWindows.Count > 0)
            {
                sidebarWindow = _detectedWindows[0];
            }

            _sidebarWindows = sidebarWindow != null
                ? new List<WindowInfo> { sidebarWindow }
                : new List<WindowInfo>();

            AccountsListBox.ItemsSource = null;
            AccountsListBox.ItemsSource = _sidebarWindows;
            AccountsListBox.SelectedItem = _sidebarWindows.FirstOrDefault();
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
            if (!_exitConfirmed)
            {
                if (HasRunningDofusClients())
                {
                    var result = MessageBox.Show(
                        "Al cerrar DofusMaster se cerrarán todas las cuentas de Dofus abiertas.\n\n¿Deseas continuar?",
                        "Confirmar salida",
                        MessageBoxButton.YesNo,
                        MessageBoxImage.Warning,
                        MessageBoxResult.No);

                    if (result != MessageBoxResult.Yes)
                    {
                        e.Cancel = true;
                        _isExiting = false;
                        return;
                    }
                }

                _exitConfirmed = true;
                _isExiting = true;
            }

            _windowEmbeddingService.RestoreEmbeddedWindow();
            CloseAllDofusClients();

            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = false;
                _notifyIcon.Dispose();
            }

            base.OnClosing(e);
        }

        protected override void OnClosed(EventArgs e)
        {
            _windowEmbeddingService.Dispose();
            base.OnClosed(e);
        }

        private void CloseAllDofusClients()
        {
            List<uint> processIds;

            try
            {
                processIds = _windowManager
                    .GetDofusWindows()
                    .Select(w => w.ProcessId)
                    .Distinct()
                    .ToList();
            }
            catch
            {
                return;
            }

            if (!processIds.Any())
            {
                return;
            }

            foreach (var pid in processIds)
            {
                TryCloseProcessGracefully(pid);
            }

            foreach (var pid in processIds)
            {
                TryForceKillProcess(pid);
            }
        }

        private bool HasRunningDofusClients()
        {
            try
            {
                return _windowManager.GetDofusWindows().Any();
            }
            catch
            {
                return false;
            }
        }

        private static void TryCloseProcessGracefully(uint processId)
        {
            try
            {
                var process = Process.GetProcessById((int)processId);
                if (process.HasExited)
                {
                    return;
                }

                // Intento de cierre normal para evitar corrupción de estado del cliente.
                if (process.CloseMainWindow())
                {
                    process.WaitForExit(1200);
                }
            }
            catch
            {
                // Ignorar; se intentará cierre forzado después.
            }
        }

        private static void TryForceKillProcess(uint processId)
        {
            try
            {
                var process = Process.GetProcessById((int)processId);
                if (process.HasExited)
                {
                    return;
                }

                process.Kill(entireProcessTree: true);
                process.WaitForExit(1200);
            }
            catch
            {
                // Ignorar para no bloquear el cierre de la app.
            }
        }
    }
}

