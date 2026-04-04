using System;
using System.Windows;

namespace DofusTabs.UI
{
    public sealed class SettingsViewState
    {
        public string SelectedAccountLabel { get; init; } = "Sin cuenta seleccionada";
        public string SelectedAccountStateLabel { get; init; } = "Selecciona una cuenta en la sidebar";
        public bool HasSelectedAccount { get; init; }
        public bool SelectedAccountEnabled { get; init; }
        public bool CanMoveUp { get; init; }
        public bool CanMoveDown { get; init; }
        public bool OverlayVisible { get; init; }
        public bool OverlayCompact { get; init; }
    }

    public partial class SettingsWindow : Window
    {
        private readonly Action _onToggleEnable;
        private readonly Action _onMoveUp;
        private readonly Action _onMoveDown;
        private readonly Action _onToggleOverlay;
        private readonly Action _onToggleOverlayCompact;

        public SettingsWindow(
            Action onToggleEnable,
            Action onMoveUp,
            Action onMoveDown,
            Action onToggleOverlay,
            Action onToggleOverlayCompact)
        {
            _onToggleEnable = onToggleEnable;
            _onMoveUp = onMoveUp;
            _onMoveDown = onMoveDown;
            _onToggleOverlay = onToggleOverlay;
            _onToggleOverlayCompact = onToggleOverlayCompact;

            InitializeComponent();
        }

        public void UpdateState(SettingsViewState state)
        {
            SelectedAccountText.Text = state.SelectedAccountLabel;
            SelectedAccountStateText.Text = state.SelectedAccountStateLabel;

            ToggleEnableButton.IsEnabled = state.HasSelectedAccount;
            ToggleEnableButton.Content = state.SelectedAccountEnabled
                ? "Deshabilitar cuenta"
                : "Habilitar cuenta";

            MoveUpButton.IsEnabled = state.HasSelectedAccount && state.CanMoveUp;
            MoveDownButton.IsEnabled = state.HasSelectedAccount && state.CanMoveDown;

            ToggleOverlayButton.Content = state.OverlayVisible
                ? "Ocultar overlay"
                : "Mostrar overlay";

            ToggleOverlayCompactButton.Content = state.OverlayCompact
                ? "Modo completo"
                : "Modo compacto";

            OverlayStateText.Text = state.OverlayVisible
                ? "Overlay visible"
                : "Overlay oculto";
        }

        private void ToggleEnableButton_Click(object sender, RoutedEventArgs e) => _onToggleEnable();

        private void MoveUpButton_Click(object sender, RoutedEventArgs e) => _onMoveUp();

        private void MoveDownButton_Click(object sender, RoutedEventArgs e) => _onMoveDown();

        private void ToggleOverlayButton_Click(object sender, RoutedEventArgs e) => _onToggleOverlay();

        private void ToggleOverlayCompactButton_Click(object sender, RoutedEventArgs e) => _onToggleOverlayCompact();

        private void CloseButton_Click(object sender, RoutedEventArgs e) => Close();
    }
}
