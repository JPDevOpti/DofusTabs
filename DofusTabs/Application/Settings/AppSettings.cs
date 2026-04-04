using System.Collections.Generic;
using System.Windows.Input;
using DofusTabs.Domain;

namespace DofusTabs.Application.Settings
{
    public sealed class AppSettings
    {
        public HotkeyBinding NextHotkey { get; set; } =
            new HotkeyBinding(ModifierKeys.Alt, Key.Tab);

        public HotkeyBinding PreviousHotkey { get; set; } =
            new HotkeyBinding(ModifierKeys.Alt | ModifierKeys.Shift, Key.Tab);

        public bool ShowSidebarNames { get; set; } = false;

        public List<InstanceSettings> Instances { get; set; } = new List<InstanceSettings>();
    }

    public sealed class InstanceSettings
    {
        public uint ProcessId { get; set; }
        public string Title { get; set; } = string.Empty;
        public bool IsEnabled { get; set; } = true;
        public int DisplayOrder { get; set; }
        public HotkeyBinding? IndividualHotkey { get; set; }
    }
}
