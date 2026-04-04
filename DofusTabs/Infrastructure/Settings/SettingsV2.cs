using System.Collections.Generic;

namespace DofusTabs.Infrastructure.Settings
{
    /// <summary>Shape del JSON de settings en versión 2 (con SchemaVersion e Instances).</summary>
    internal sealed class SettingsV2
    {
        public int SchemaVersion { get; set; } = 2;

        public string NextHotkeyModifiers     { get; set; } = "Alt";
        public string NextHotkeyKey           { get; set; } = "Tab";
        public string PreviousHotkeyModifiers { get; set; } = "Alt,Shift";
        public string PreviousHotkeyKey       { get; set; } = "Tab";

        public List<InstanceSettingsV2> Instances { get; set; } = new();
    }

    internal sealed class InstanceSettingsV2
    {
        public uint   ProcessId       { get; set; }
        public string Title           { get; set; } = string.Empty;
        public bool   IsEnabled       { get; set; } = true;
        public int    DisplayOrder    { get; set; }
        public string HotkeyModifiers { get; set; } = string.Empty;
        public string HotkeyKey       { get; set; } = string.Empty;
    }
}
