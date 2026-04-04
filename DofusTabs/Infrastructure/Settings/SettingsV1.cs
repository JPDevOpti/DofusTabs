using System.Collections.Generic;

namespace DofusTabs.Infrastructure.Settings
{
    /// <summary>Shape del JSON de settings en versión 1 (sin SchemaVersion).</summary>
    internal sealed class SettingsV1
    {
        public string NextHotkeyModifiers     { get; set; } = "Alt";
        public string NextHotkeyKey           { get; set; } = "Tab";
        public string PreviousHotkeyModifiers { get; set; } = "Alt,Shift";
        public string PreviousHotkeyKey       { get; set; } = "Tab";
        public List<WindowSettingsV1> Windows { get; set; } = new();
    }

    internal sealed class WindowSettingsV1
    {
        public string Title          { get; set; } = string.Empty;
        public uint   ProcessId      { get; set; }
        public bool   IsEnabled      { get; set; } = true;
        public string IndividualHotkey { get; set; } = string.Empty;
        public int    DisplayOrder   { get; set; }
        public string HotkeyModifiers { get; set; } = string.Empty;
        public string HotkeyKey      { get; set; } = string.Empty;
    }
}
