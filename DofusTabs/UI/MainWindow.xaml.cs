using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using DofusTabs.Application.Services;
using DofusTabs.Application.Settings;
using DofusTabs.Diagnostics;
using DofusTabs.Domain;
using DofusTabs.Infrastructure.Discovery;
using DofusTabs.Infrastructure.Win32;
using Drawing = System.Drawing;
using Forms = System.Windows.Forms;

namespace DofusTabs.UI
{
    public partial class MainWindow : Window
    {
        private readonly IGameDiscoveryService _discovery;
        private readonly IEmbeddingService _embedding;
        private readonly IHotkeyService _hotkeys;
        private readonly ISettingsService _settings;
        private readonly IProcessWatcher _watcher;
        private readonly GameDiscoveryService _discoveryImpl;

        private List<GameInstance> _instances = new();
        private uint? _activeProcessId;
        private uint _attachedGameThreadId;
        private HotkeyBinding _nextHotkey     = new(ModifierKeys.Alt, Key.Tab);
        private HotkeyBinding _previousHotkey = new(ModifierKeys.Alt | ModifierKeys.Shift, Key.Tab);
        private bool _showSidebarNames;
        private Forms.NotifyIcon? _notifyIcon;
        private SettingsWindow? _settingsWindow;
        private bool _isExiting;
        private bool _exitConfirmed;

        public MainWindow(
            IGameDiscoveryService discovery,
            IEmbeddingService embedding,
            IHotkeyService hotkeys,
            ISettingsService settings,
            IProcessWatcher watcher)
        {
            _discovery     = discovery;
            _embedding     = embedding;
            _hotkeys       = hotkeys;
            _settings      = settings;
            _watcher       = watcher;
            _discoveryImpl = (GameDiscoveryService)discovery;

            InitializeComponent();
            SetupTrayIcon();

            Loaded       += MainWindow_Loaded;
            StateChanged += MainWindow_StateChanged;
        }

        // ── Inicialización ───────────────────────────────────────────────────

        private void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            // Inicializar hotkeys (necesita el HWND, disponible solo después de Loaded)
            _hotkeys.Initialize(this);

            var savedSettings = _settings.Load();

            _nextHotkey     = savedSettings.NextHotkey;
            _previousHotkey = savedSettings.PreviousHotkey;
            _showSidebarNames = savedSettings.ShowSidebarNames;
            ApplySidebarVisualOptions();

            _hotkeys.Register(_hotkeys.NextHotkeyId,     _nextHotkey);
            _hotkeys.Register(_hotkeys.PreviousHotkeyId, _previousHotkey);

            _hotkeys.NextRequested               += OnNextRequested;
            _hotkeys.PreviousRequested           += OnPreviousRequested;
            _hotkeys.InstanceActivationRequested += OnInstanceActivationRequested;

            GameHostPanel.Resize += (_, _) => _embedding.Resize(
                Math.Max(1, GameHostPanel.ClientSize.Width),
                Math.Max(1, GameHostPanel.ClientSize.Height));
            GameHostPanel.Enter += (_, _) => FocusCurrentEmbeddedWindow();

            _watcher.ProcessAppeared    += (_, _) => Dispatcher.Invoke(() => Refresh(selectFallback: false));
            _watcher.ProcessDisappeared += (_, _) => Dispatcher.Invoke(() => Refresh(selectFallback: false));
            _watcher.Start();

            // Primer refresh + aplicar configuración guardada
            Refresh(selectFallback: true);
            _discoveryImpl.ApplyPersistedSettings(savedSettings.Instances);
            ApplyPerInstanceHotkeys(savedSettings.Instances);
            Refresh(selectFallback: !_activeProcessId.HasValue);
            UpdateSettingsWindowState();

            AppLogger.Info("MainWindow cargada");
        }

        // ── Hotkeys ──────────────────────────────────────────────────────────

        private void OnNextRequested()
        {
            var enabled = _instances.Where(i => i.IsEnabled).OrderBy(i => i.DisplayOrder).ToList();
            if (!enabled.Any()) return;
            int idx = enabled.FindIndex(i => i.ProcessId == _activeProcessId);
            int next = (idx + 1) % enabled.Count;
            Dispatcher.Invoke(() => ActivateAccount(enabled[next]));
        }

        private void OnPreviousRequested()
        {
            var enabled = _instances.Where(i => i.IsEnabled).OrderBy(i => i.DisplayOrder).ToList();
            if (!enabled.Any()) return;
            int idx = enabled.FindIndex(i => i.ProcessId == _activeProcessId);
            int prev = (idx - 1 + enabled.Count) % enabled.Count;
            Dispatcher.Invoke(() => ActivateAccount(enabled[prev]));
        }

        private void OnInstanceActivationRequested(uint processId)
        {
            var instance = _instances.FirstOrDefault(i => i.ProcessId == processId);
            if (instance != null)
                Dispatcher.Invoke(() => ActivateAccount(instance));
        }

        // ── Refresh ──────────────────────────────────────────────────────────

        private void RefreshButton_Click(object sender, RoutedEventArgs e) =>
            Refresh(selectFallback: true);

        private void Refresh(bool selectFallback)
        {
            _instances = SortSidebarInstances(_discovery.GetSnapshot());

            var activePids = _instances.Select(i => i.ProcessId).ToHashSet();
            foreach (var pid in _embedding.EmbeddedProcessIds)
            {
                if (!activePids.Contains(pid))
                {
                    if (_activeProcessId == pid) _activeProcessId = null;
                    _embedding.Restore(pid);
                }
            }

            UpdateActiveFlags();
            UpdateSidebar();

            if (!_instances.Any())
            {
                EmptyHostText.Visibility = Visibility.Visible;
                return;
            }

            if (selectFallback && !_activeProcessId.HasValue)
            {
                var fallback = _instances
                    .Where(i => i.IsEnabled)
                    .OrderBy(i => i.DisplayOrder)
                    .FirstOrDefault();

                if (fallback != null)
                    ActivateAccount(fallback);
            }
        }

        // ── Activación de cuenta ─────────────────────────────────────────────

        private void AccountBubble_Click(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.Tag is GameInstance instance)
            {
                AccountsListBox.SelectedItem = instance;
                if (!instance.IsEnabled)
                    return;

                ActivateAccount(instance);
            }
        }

        private void AccountBubble_PreviewMouseRightButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (sender is Button btn && btn.Tag is GameInstance instance)
                AccountsListBox.SelectedItem = instance;
        }

        private void AccountContextMenu_Opened(object sender, RoutedEventArgs e)
        {
            if (sender is not ContextMenu menu)
                return;

            if (menu.Items.OfType<MenuItem>().FirstOrDefault() is not MenuItem toggleItem)
                return;

            if (menu.PlacementTarget is not FrameworkElement target || target.DataContext is not GameInstance instance)
                return;

            toggleItem.Header = instance.IsEnabled
                ? "Deshabilitar cuenta"
                : "Habilitar cuenta";
            toggleItem.Tag = instance;
        }

        private void ToggleAccountEnabledFromSidebar_Click(object sender, RoutedEventArgs e)
        {
            if (sender is not MenuItem item || item.Tag is not GameInstance instance)
                return;

            var current = _instances.FirstOrDefault(i => i.ProcessId == instance.ProcessId);
            if (current == null)
                return;

            AccountsListBox.SelectedItem = current;
            ToggleSelectedAccountEnabled();
        }

        private void ActivateAccount(GameInstance instance)
        {
            if (!instance.IsEnabled)
                return;

            if (_activeProcessId.HasValue && _activeProcessId.Value == instance.ProcessId)
            {
                FocusCurrentEmbeddedWindow();
                return;
            }

            if (!_discovery.TryGetWindowHandle(instance.ProcessId, out var gameHandle))
            {
                AppLogger.Warn($"ActivateAccount: handle no encontrado para pid={instance.ProcessId}");
                return;
            }

            bool embedded = _embedding.TryEmbed(
                instance.ProcessId,
                gameHandle,
                GameHostPanel.Handle,
                Math.Max(1, GameHostPanel.ClientSize.Width),
                Math.Max(1, GameHostPanel.ClientSize.Height));

            if (!embedded)
            {
                AppLogger.Warn($"ActivateAccount: TryEmbed falló para pid={instance.ProcessId}");
                return;
            }

            _activeProcessId = instance.ProcessId;
            UpdateActiveFlags();
            UpdateSidebar();
            EmptyHostText.Visibility = Visibility.Collapsed;
            TryFocusEmbeddedWindow(gameHandle);
            AppLogger.Info($"Cuenta activa: {instance.CharacterName} (pid={instance.ProcessId})");
        }

        private void ToggleSelectedAccountEnabledButton_Click(object sender, RoutedEventArgs e) =>
            ToggleSelectedAccountEnabled();

        private void ToggleSelectedAccountEnabled()
        {
            var selected = GetSelectedInstance();
            if (selected == null)
                return;

            selected.IsEnabled = !selected.IsEnabled;

            if (!selected.IsEnabled)
            {
                _hotkeys.UnregisterForInstance(selected.ProcessId);

                if (_activeProcessId == selected.ProcessId)
                    _activeProcessId = null;
            }
            else if (selected.IndividualHotkey != null && !selected.IndividualHotkey.IsEmpty)
            {
                _hotkeys.RegisterForInstance(selected.ProcessId, selected.IndividualHotkey);
            }

            UpdateActiveFlags();
            UpdateSidebar();

            if (!_activeProcessId.HasValue)
            {
                var fallback = _instances
                    .Where(i => i.IsEnabled)
                    .OrderBy(i => i.DisplayOrder)
                    .FirstOrDefault();

                if (fallback != null)
                {
                    ActivateAccount(fallback);
                }
                else
                {
                    EmptyHostText.Visibility = Visibility.Visible;
                }
            }

            PersistInteractiveState("toggle enable");
            UpdateSettingsWindowState();
        }

        private void MoveAccountUpButton_Click(object sender, RoutedEventArgs e) =>
            MoveSelectedAccount(delta: -1);

        private void MoveAccountDownButton_Click(object sender, RoutedEventArgs e) =>
            MoveSelectedAccount(delta: 1);

        private void MoveSelectedAccount(int delta)
        {
            var selected = GetSelectedInstance();
            if (selected == null)
                return;

            var ordered = _instances
                .OrderBy(i => i.DisplayOrder)
                .ThenBy(i => i.CharacterName)
                .ToList();

            int currentIndex = ordered.FindIndex(i => i.ProcessId == selected.ProcessId);
            if (currentIndex < 0)
                return;

            int targetIndex = currentIndex + delta;
            if (targetIndex < 0 || targetIndex >= ordered.Count)
                return;

            (ordered[currentIndex], ordered[targetIndex]) = (ordered[targetIndex], ordered[currentIndex]);

            for (int i = 0; i < ordered.Count; i++)
                ordered[i].DisplayOrder = i;

            _instances = ordered;
            UpdateSidebar();
            AccountsListBox.SelectedItem = _instances.FirstOrDefault(i => i.ProcessId == selected.ProcessId);
            PersistInteractiveState(delta < 0 ? "move up" : "move down");
            UpdateSettingsWindowState();
        }

        private void RestoreEmbeddedClientButton_Click(object sender, RoutedEventArgs e)
        {
            DetachGameThread();
            if (_activeProcessId.HasValue)
                _embedding.Restore(_activeProcessId.Value);

            _activeProcessId = null;
            UpdateActiveFlags();
            UpdateSidebar();
            EmptyHostText.Visibility = Visibility.Visible;
        }

        // ── Settings ─────────────────────────────────────────────────────────

        private void SettingsButton_Click(object sender, RoutedEventArgs e)
        {
            OpenSettingsWindow();
        }

        private void ApplyPerInstanceHotkeys(List<InstanceSettings> saved)
        {
            foreach (var s in saved)
            {
                if (s.IndividualHotkey != null && !s.IndividualHotkey.IsEmpty)
                    _hotkeys.RegisterForInstance(s.ProcessId, s.IndividualHotkey);
            }
        }

        // ── UI helpers ───────────────────────────────────────────────────────

        private void UpdateActiveFlags()
        {
            foreach (var inst in _instances)
                inst.IsActive = _activeProcessId.HasValue && inst.ProcessId == _activeProcessId.Value;

            AccountsSummaryText.Text = $"{_instances.Count} cuentas";
        }

        private void UpdateSidebar()
        {
            _instances = SortSidebarInstances(_instances);

            var selectedProcessId = (AccountsListBox.SelectedItem as GameInstance)?.ProcessId;

            AccountsListBox.ItemsSource = null;
            AccountsListBox.ItemsSource = _instances;

            if (_activeProcessId.HasValue)
            {
                var active = _instances.FirstOrDefault(i => i.ProcessId == _activeProcessId.Value);
                if (active != null)
                    AccountsListBox.SelectedItem = active;
            }
            else if (selectedProcessId.HasValue)
            {
                var selected = _instances.FirstOrDefault(i => i.ProcessId == selectedProcessId.Value);
                if (selected != null)
                    AccountsListBox.SelectedItem = selected;
            }

            UpdateSettingsWindowState();
        }

        private void AccountsListBox_SelectionChanged(object sender, SelectionChangedEventArgs e) =>
            UpdateSettingsWindowState();

        private void OpenSettingsWindow()
        {
            if (_settingsWindow != null)
            {
                UpdateSettingsWindowState();
                _settingsWindow.Show();
                _settingsWindow.Activate();
                return;
            }

            _settingsWindow = new SettingsWindow(
                onHotkeysChanged: (next, prev) =>
                {
                    _nextHotkey     = next;
                    _previousHotkey = prev;
                    _hotkeys.Register(_hotkeys.NextHotkeyId,     next);
                    _hotkeys.Register(_hotkeys.PreviousHotkeyId, prev);
                    PersistInteractiveState("hotkeys");
                },
                onCaptureModeChanged: suspended => _hotkeys.SetSuspended(suspended),
                onShowSidebarNamesChanged: showNames =>
                {
                    _showSidebarNames = showNames;
                    ApplySidebarVisualOptions();
                    PersistInteractiveState("sidebar names");
                })
            {
                Owner = this
            };

            _settingsWindow.Closed += (_, _) => _settingsWindow = null;

            UpdateSettingsWindowState();
            _settingsWindow.Show();
            _settingsWindow.Activate();
        }

        private SettingsViewState BuildSettingsViewState()
        {
            return new SettingsViewState
            {
                NextHotkey                = _nextHotkey,
                PreviousHotkey            = _previousHotkey,
                ShowSidebarNames          = _showSidebarNames,
            };
        }

        private void UpdateSettingsWindowState()
        {
            if (_settingsWindow == null)
                return;

            _settingsWindow.UpdateState(BuildSettingsViewState());
        }

        private void FocusCurrentEmbeddedWindow()
        {
            if (!_activeProcessId.HasValue)
                return;

            if (_discovery.TryGetWindowHandle(_activeProcessId.Value, out var gameHandle))
                TryFocusEmbeddedWindow(gameHandle);
        }

        private void TryFocusEmbeddedWindow(IntPtr gameHandle)
        {
            if (gameHandle == IntPtr.Zero)
                return;

            IntPtr mainHandle = new WindowInteropHelper(this).Handle;
            if (mainHandle == IntPtr.Zero)
                return;

            uint uiThreadId  = User32.GetCurrentThreadId();
            uint gameThreadId = User32.GetWindowThreadProcessId(gameHandle, out _);
            if (gameThreadId == 0)
                return;

            EnsureGameThreadAttached(uiThreadId, gameThreadId);
            User32.SetForegroundWindow(mainHandle);
            User32.SetFocus(gameHandle);
        }

        private void EnsureGameThreadAttached(uint uiThreadId, uint gameThreadId)
        {
            if (gameThreadId == uiThreadId) return;

            if (_attachedGameThreadId != 0 && _attachedGameThreadId != gameThreadId)
            {
                User32.AttachThreadInput(uiThreadId, _attachedGameThreadId, false);
                _attachedGameThreadId = 0;
            }

            if (_attachedGameThreadId == 0)
            {
                User32.AttachThreadInput(uiThreadId, gameThreadId, true);
                _attachedGameThreadId = gameThreadId;
            }
        }

        private void DetachGameThread()
        {
            if (_attachedGameThreadId == 0) return;
            User32.AttachThreadInput(User32.GetCurrentThreadId(), _attachedGameThreadId, false);
            _attachedGameThreadId = 0;
        }

        private GameInstance? GetSelectedInstance()
        {
            if (AccountsListBox.SelectedItem is GameInstance selected)
                return selected;

            return null;
        }

        private void ApplySidebarVisualOptions()
        {
            AccountsListBox.Tag = _showSidebarNames;
        }

        private static List<GameInstance> SortSidebarInstances(IEnumerable<GameInstance> instances)
        {
            return instances
                .OrderBy(i => i.IsEnabled ? 0 : 1)
                .ThenBy(i => i.DisplayOrder)
                .ThenBy(i => i.CharacterName)
                .ToList();
        }

        // ── Tray ─────────────────────────────────────────────────────────────

        private void SetupTrayIcon()
        {
            _notifyIcon = new Forms.NotifyIcon();
            try
            {
                var exePath = Environment.ProcessPath
                    ?? Process.GetCurrentProcess().MainModule?.FileName;
                if (!string.IsNullOrWhiteSpace(exePath))
                    _notifyIcon.Icon = Drawing.Icon.ExtractAssociatedIcon(exePath);
            }
            catch { }

            _notifyIcon.Text    = "DofusTabs (en ejecución)";
            _notifyIcon.Visible = true;
            _notifyIcon.DoubleClick += (_, _) => RestoreFromTray();

            var menu = new Forms.ContextMenuStrip();
            menu.Items.Add("Mostrar", null, (_, _) => RestoreFromTray());
            menu.Items.Add("Salir",   null, (_, _) => ExitFromTray());
            _notifyIcon.ContextMenuStrip = menu;
        }

        private void RestoreFromTray()
        {
            Show();
            ShowInTaskbar = true;
            WindowState = WindowState.Normal;
            Activate();
            FocusCurrentEmbeddedWindow();
        }

        private void ExitFromTray()
        {
            _isExiting = true;
            Close();
        }

        private void MainWindow_StateChanged(object? sender, EventArgs e)
        {
            if (WindowState == WindowState.Minimized && !_isExiting)
            {
                Hide();
                ShowInTaskbar = false;
                if (_notifyIcon != null) _notifyIcon.Visible = true;
            }
            else if (WindowState == WindowState.Normal)
            {
                ShowInTaskbar = true;
            }
        }

        // ── Cierre ───────────────────────────────────────────────────────────

        protected override void OnClosing(CancelEventArgs e)
        {
            if (!_exitConfirmed)
            {
                if (HasRunningDofusClients())
                {
                    var result = MessageBox.Show(
                        "Al cerrar DofusTabs se cerrarán todas las cuentas de Dofus abiertas.\n\n¿Deseas continuar?",
                        "Confirmar salida",
                        MessageBoxButton.YesNo, MessageBoxImage.Warning, MessageBoxResult.No);

                    if (result != MessageBoxResult.Yes)
                    {
                        e.Cancel = true;
                        _isExiting = false;
                        return;
                    }
                }
                _exitConfirmed = true;
                _isExiting     = true;
            }

            SaveCurrentSettings();

            if (_settingsWindow != null)
            {
                _settingsWindow.Close();
                _settingsWindow = null;
            }

            DetachGameThread();
            _embedding.RestoreAll();
            CloseAllDofusClients();

            _watcher.Stop();
            _hotkeys.Dispose();

            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = false;
                _notifyIcon.Dispose();
            }

            base.OnClosing(e);
        }

        private void SaveCurrentSettings()
        {
            PersistInteractiveState("shutdown");
        }

        private void PersistInteractiveState(string source)
        {
            try
            {
                var saved = _settings.Load();
                saved.NextHotkey     = _nextHotkey;
                saved.PreviousHotkey = _previousHotkey;
                saved.ShowSidebarNames = _showSidebarNames;
                saved.Instances.Clear();
                foreach (var inst in _instances)
                {
                    saved.Instances.Add(new InstanceSettings
                    {
                        ProcessId        = inst.ProcessId,
                        Title            = inst.WindowTitle,
                        IsEnabled        = inst.IsEnabled,
                        DisplayOrder     = inst.DisplayOrder,
                        IndividualHotkey = inst.IndividualHotkey,
                    });
                }

                _settings.Save(saved);
                AppLogger.Info($"Settings persistidos ({source})");
            }
            catch (Exception ex)
            {
                AppLogger.Error(ex, $"Error guardando settings ({source})");
            }
        }

        private bool HasRunningDofusClients()
        {
            try { return _discovery.GetSnapshot(preserveState: false).Any(); }
            catch { return false; }
        }

        private void CloseAllDofusClients()
        {
            List<uint> pids;
            try
            {
                pids = _discovery.GetSnapshot(preserveState: false)
                    .Select(i => i.ProcessId).Distinct().ToList();
            }
            catch { return; }

            foreach (var pid in pids) TryCloseGracefully(pid);
            foreach (var pid in pids) TryForceKill(pid);
        }

        private static void TryCloseGracefully(uint pid)
        {
            try
            {
                var p = Process.GetProcessById((int)pid);
                if (!p.HasExited && p.CloseMainWindow())
                    p.WaitForExit(1200);
            }
            catch { }
        }

        private static void TryForceKill(uint pid)
        {
            try
            {
                var p = Process.GetProcessById((int)pid);
                if (!p.HasExited)
                {
                    p.Kill(entireProcessTree: true);
                    p.WaitForExit(1200);
                }
            }
            catch { }
        }
    }
}
