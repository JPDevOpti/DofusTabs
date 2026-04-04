using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using DofusTabs.Application.Services;
using DofusTabs.Domain;
using DofusTabs.Infrastructure.Win32;

namespace DofusTabs.UI
{
    public partial class OverlayWindow : Window
    {
        private readonly IGameDiscoveryService _discovery;

        private List<GameInstance> _cachedInstances = new();
        private GameInstance? _currentActive;
        private readonly DispatcherTimer _focusTimer;
        private readonly Dictionary<uint, Border> _borderCache = new();
        private uint? _lastActiveId;

        private bool _isCompact;
        private static readonly SolidColorBrush ActiveBrush =
            new SolidColorBrush(Color.FromRgb(143, 122, 78));
        private static readonly SolidColorBrush ActiveBorderBrush =
            new SolidColorBrush(Color.FromRgb(111, 90, 52));

        public event Action? OnOverlayHidden;
        public event Action<bool>? OnCompactChanged;

        public bool IsCompact => _isCompact;

        public OverlayWindow(IGameDiscoveryService discovery)
        {
            _discovery = discovery;

            InitializeComponent();

            _focusTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(50) };
            _focusTimer.Tick += FocusTimer_Tick;

            Loaded += OverlayWindow_Loaded;
            ApplyLayoutMode();
        }

        private void OverlayWindow_Loaded(object sender, RoutedEventArgs e)
        {
            _focusTimer.Start();
        }

        // ── Refresh público ──────────────────────────────────────────────────

        public void RefreshInstanceList(IReadOnlyList<GameInstance> instances)
        {
            var enabled = instances.Where(i => i.IsEnabled).ToList();

            bool changed = _cachedInstances.Count != enabled.Count
                || _cachedInstances.Select(i => i.ProcessId)
                    .Zip(enabled.Select(i => i.ProcessId))
                    .Any(pair => pair.First != pair.Second);

            if (!changed) return;

            _cachedInstances = enabled;
            _borderCache.Clear();
            _lastActiveId = null;
            CharactersList.ItemsSource = _cachedInstances;
            HighlightActiveInstance();
        }

        // ── Timer de foco ────────────────────────────────────────────────────

        private void FocusTimer_Tick(object? sender, EventArgs e)
        {
            IntPtr foreground = User32.GetForegroundWindow();
            var active = _cachedInstances.FirstOrDefault(i =>
            {
                _discovery.TryGetWindowHandle(i.ProcessId, out var h);
                return h == foreground;
            });

            uint? activeId = active?.ProcessId;
            if (activeId == _lastActiveId) return;

            _currentActive = active;
            HighlightActiveInstance();
        }

        // ── Eventos de controles ─────────────────────────────────────────────

        private void CompactToggleButton_Click(object sender, RoutedEventArgs e)
        {
            _isCompact = !_isCompact;
            ApplyLayoutMode();
            HighlightActiveInstance();
            OnCompactChanged?.Invoke(_isCompact);
        }

        private void CloseOverlayButton_Click(object sender, RoutedEventArgs e)
        {
            Hide();
            OnOverlayHidden?.Invoke();
        }

        private void Border_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.Source == sender && e.ButtonState == MouseButtonState.Pressed)
                DragMove();
        }

        private void CharacterItem_Click(object sender, MouseButtonEventArgs e)
        {
            if (sender is Border border && border.Tag is GameInstance instance && instance.IsEnabled)
            {
                if (_discovery.TryGetWindowHandle(instance.ProcessId, out var handle))
                {
                    if (User32.IsIconic(handle))
                        User32.ShowWindow(handle, User32.SW_RESTORE);
                    User32.SetForegroundWindow(handle);
                }
                e.Handled = true;
            }
        }

        // ── Resaltado activo ─────────────────────────────────────────────────

        private void HighlightActiveInstance()
        {
            uint? currentId = _currentActive?.ProcessId;
            if (currentId == _lastActiveId) return;

            uint? previousId = _lastActiveId;
            _lastActiveId = currentId;

            foreach (var item in CharactersList.Items)
            {
                if (item is not GameInstance instance) continue;
                if (instance.ProcessId != currentId && instance.ProcessId != previousId) continue;

                if (!_borderCache.TryGetValue(instance.ProcessId, out var border))
                {
                    var container = CharactersList.ItemContainerGenerator
                        .ContainerFromItem(item) as FrameworkElement;
                    if (container != null)
                    {
                        border = FindVisualChild<Border>(container);
                        if (border != null)
                            _borderCache[instance.ProcessId] = border;
                    }
                }

                if (border == null) continue;

                if (instance.ProcessId == currentId)
                {
                    border.Background       = ActiveBrush;
                    border.BorderBrush      = ActiveBorderBrush;
                    border.BorderThickness  = new Thickness(1);
                }
                else
                {
                    border.Background      = Brushes.Transparent;
                    border.BorderThickness = new Thickness(0);
                }
            }
        }

        // ── Layout ───────────────────────────────────────────────────────────

        public void SetCompactMode(bool compact)
        {
            if (_isCompact == compact) return;
            _isCompact = compact;
            ApplyLayoutMode();
            HighlightActiveInstance();
            OnCompactChanged?.Invoke(_isCompact);
        }

        private void ApplyLayoutMode()
        {
            var templateKey = _isCompact ? "CompactItemTemplate" : "FullItemTemplate";
            CharactersList.ItemTemplate = (DataTemplate)FindResource(templateKey);

            if (OverlayContainer != null)
                OverlayContainer.Padding = _isCompact ? new Thickness(3) : new Thickness(4);

            _borderCache.Clear();
            _lastActiveId = null;
            UpdateCompactToggleVisuals();
        }

        private void UpdateCompactToggleVisuals()
        {
            if (CompactToggleText == null || CompactToggleButton == null) return;
            CompactToggleText.Text = _isCompact ? "→" : "←";
            CompactToggleButton.ToolTip = _isCompact ? "Mostrar nombres" : "Mostrar solo iconos";
        }

        // ── Posición ─────────────────────────────────────────────────────────

        public void SetPosition(double x, double y) { Left = x; Top = y; }

        protected override void OnLocationChanged(EventArgs e)
        {
            base.OnLocationChanged(e);
            // La posición del overlay ya no se guarda aquí: el llamador usa ISettingsService
        }

        protected override void OnClosed(EventArgs e)
        {
            _focusTimer.Stop();
            base.OnClosed(e);
        }

        // ── Utilidad ─────────────────────────────────────────────────────────

        private static T? FindVisualChild<T>(DependencyObject parent) where T : DependencyObject
        {
            for (int i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
            {
                var child = VisualTreeHelper.GetChild(parent, i);
                if (child is T t) return t;
                var found = FindVisualChild<T>(child);
                if (found != null) return found;
            }
            return null;
        }
    }
}
