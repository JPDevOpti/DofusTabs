using System;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using DofusTabs.Domain;

namespace DofusTabs.UI
{
    public sealed class SettingsViewState
    {
        public HotkeyBinding NextHotkey { get; init; } = new(ModifierKeys.Alt, Key.Tab);
        public HotkeyBinding PreviousHotkey { get; init; } = new(ModifierKeys.Alt | ModifierKeys.Shift, Key.Tab);
        public HotkeyBinding PrimaryHotkey { get; init; } = new(ModifierKeys.Alt, Key.Home);
        public bool ShowSidebarNames { get; init; }
    }

    public partial class SettingsWindow : Window
    {
        private readonly Action<HotkeyBinding, HotkeyBinding, HotkeyBinding> _onHotkeysChanged;
        private readonly Action<bool> _onCaptureModeChanged;
        private readonly Action<bool> _onShowSidebarNamesChanged;

        private HotkeyBinding _currentNext;
        private HotkeyBinding _currentPrev;
        private HotkeyBinding _currentPrimary;

        private enum CaptureTarget { None, Next, Previous, Primary }
        private CaptureTarget _capturing = CaptureTarget.None;
        private bool _updatingFromState;

        private static readonly SolidColorBrush _captureBackground  = new(Color.FromRgb(0x2A, 0x2A, 0x1A));
        private static readonly SolidColorBrush _captureBorder       = new(Color.FromRgb(0xD5, 0xC1, 0x7A));
        private static readonly SolidColorBrush _normalBackground    = new(Color.FromRgb(0x2A, 0x2F, 0x2F));
        private static readonly SolidColorBrush _normalBorder        = new(Color.FromRgb(0x3E, 0x45, 0x45));

        public SettingsWindow(
            Action<HotkeyBinding, HotkeyBinding, HotkeyBinding> onHotkeysChanged,
            Action<bool> onCaptureModeChanged,
            Action<bool> onShowSidebarNamesChanged)
        {
            _onHotkeysChanged     = onHotkeysChanged;
            _onCaptureModeChanged = onCaptureModeChanged;
            _onShowSidebarNamesChanged = onShowSidebarNamesChanged;

            _currentNext = new HotkeyBinding(ModifierKeys.Alt, Key.Tab);
            _currentPrev = new HotkeyBinding(ModifierKeys.Alt | ModifierKeys.Shift, Key.Tab);
            _currentPrimary = new HotkeyBinding(ModifierKeys.Alt, Key.Home);

            InitializeComponent();
        }

        public void UpdateState(SettingsViewState state)
        {
            _updatingFromState = true;
            ShowSidebarNamesCheckBox.IsChecked = state.ShowSidebarNames;
            _updatingFromState = false;

            if (_capturing == CaptureTarget.None)
            {
                _currentNext = state.NextHotkey;
                _currentPrev = state.PreviousHotkey;
                _currentPrimary = state.PrimaryHotkey;
                NextHotkeyButton.Content = state.NextHotkey.ToString();
                PrevHotkeyButton.Content = state.PreviousHotkey.ToString();
                PrimaryHotkeyButton.Content = state.PrimaryHotkey.ToString();
            }
        }

        protected override void OnPreviewKeyDown(KeyEventArgs e)
        {
            if (_capturing == CaptureTarget.None)
            {
                base.OnPreviewKeyDown(e);
                return;
            }

            e.Handled = true;

            var key = e.Key == Key.System ? e.SystemKey : e.Key;

            if (key == Key.Escape)
            {
                CancelCapture();
                return;
            }

            if (IsModifierKey(key)) return;

            CompleteCapture(new HotkeyBinding(Keyboard.Modifiers, key));
        }

        private void NextHotkeyButton_Click(object sender, RoutedEventArgs e)
        {
            if (_capturing == CaptureTarget.Next) { CancelCapture(); return; }
            if (_capturing == CaptureTarget.Previous) CancelCapture();
            StartCapture(CaptureTarget.Next);
        }

        private void PrevHotkeyButton_Click(object sender, RoutedEventArgs e)
        {
            if (_capturing == CaptureTarget.Previous) { CancelCapture(); return; }
            if (_capturing == CaptureTarget.Next) CancelCapture();
            StartCapture(CaptureTarget.Previous);
        }

        private void PrimaryHotkeyButton_Click(object sender, RoutedEventArgs e)
        {
            if (_capturing == CaptureTarget.Primary) { CancelCapture(); return; }
            if (_capturing != CaptureTarget.None) CancelCapture();
            StartCapture(CaptureTarget.Primary);
        }

        private void StartCapture(CaptureTarget target)
        {
            _capturing = target;
            _onCaptureModeChanged(true);

            var btn = target switch
            {
                CaptureTarget.Next => NextHotkeyButton,
                CaptureTarget.Previous => PrevHotkeyButton,
                CaptureTarget.Primary => PrimaryHotkeyButton,
                _ => NextHotkeyButton,
            };
            btn.Content    = "Presiona combinación...";
            btn.Background = _captureBackground;
            btn.BorderBrush = _captureBorder;

        }

        private void CancelCapture()
        {
            var btn = _capturing switch
            {
                CaptureTarget.Next => NextHotkeyButton,
                CaptureTarget.Previous => PrevHotkeyButton,
                CaptureTarget.Primary => PrimaryHotkeyButton,
                _ => NextHotkeyButton,
            };
            btn.Content = _capturing switch
            {
                CaptureTarget.Next => _currentNext.ToString(),
                CaptureTarget.Previous => _currentPrev.ToString(),
                CaptureTarget.Primary => _currentPrimary.ToString(),
                _ => string.Empty,
            };
            btn.Background  = _normalBackground;
            btn.BorderBrush = _normalBorder;

            // hint cleared(_capturing);
            _capturing = CaptureTarget.None;
            _onCaptureModeChanged(false);
        }

        private void CompleteCapture(HotkeyBinding binding)
        {
            if (_capturing == CaptureTarget.Next)
                _currentNext = binding;
            else if (_capturing == CaptureTarget.Previous)
                _currentPrev = binding;
            else
                _currentPrimary = binding;

            var btn = _capturing switch
            {
                CaptureTarget.Next => NextHotkeyButton,
                CaptureTarget.Previous => PrevHotkeyButton,
                CaptureTarget.Primary => PrimaryHotkeyButton,
                _ => NextHotkeyButton,
            };
            btn.Content     = binding.ToString();
            btn.Background  = _normalBackground;
            btn.BorderBrush = _normalBorder;

            // hint cleared(_capturing);
            _capturing = CaptureTarget.None;
            _onCaptureModeChanged(false);
            _onHotkeysChanged(_currentNext, _currentPrev, _currentPrimary);
        }


        private static bool IsModifierKey(Key key) =>
            key is Key.LeftCtrl or Key.RightCtrl
                or Key.LeftAlt  or Key.RightAlt
                or Key.LeftShift or Key.RightShift
                or Key.LWin or Key.RWin
                or Key.System;

        private void ShowSidebarNamesCheckBox_Changed(object sender, RoutedEventArgs e)
        {
            if (_updatingFromState)
                return;

            _onShowSidebarNamesChanged(ShowSidebarNamesCheckBox.IsChecked == true);
        }

        private void CloseButton_Click(object sender, RoutedEventArgs e)         => Close();
    }
}
