using System.Collections.Generic;
using System.Linq;
using System.Text.Json;

namespace DofusTabs.Infrastructure.Settings
{
    /// <summary>
    /// Migra documentos JSON de settings de versiones antiguas a la versión actual (V2).
    /// Cada migración es una función pura JsonDocument → SettingsV2.
    /// </summary>
    internal static class SettingsMigrator
    {
        private const int CurrentVersion = 2;

        public static SettingsV2 MigrateToLatest(JsonDocument doc)
        {
            int version = DetectVersion(doc);

            return version switch
            {
                1 => MigrateV1ToV2(doc),
                2 => DeserializeV2(doc),
                _ => new SettingsV2(), // versión desconocida: valores por defecto seguros
            };
        }

        private static int DetectVersion(JsonDocument doc)
        {
            if (doc.RootElement.TryGetProperty("SchemaVersion", out var v) && v.TryGetInt32(out int ver))
                return ver;
            return 1; // Sin SchemaVersion = V1
        }

        private static SettingsV2 MigrateV1ToV2(JsonDocument doc)
        {
            var v1 = JsonSerializer.Deserialize<SettingsV1>(doc.RootElement.GetRawText())
                     ?? new SettingsV1();

            var v2 = new SettingsV2
            {
                NextHotkeyModifiers     = v1.NextHotkeyModifiers,
                NextHotkeyKey           = v1.NextHotkeyKey,
                PreviousHotkeyModifiers = v1.PreviousHotkeyModifiers,
                PreviousHotkeyKey       = v1.PreviousHotkeyKey,
                OverlayX       = v1.OverlayX,
                OverlayY       = v1.OverlayY,
                OverlayVisible = v1.OverlayVisible,
                OverlayCompact = v1.OverlayCompact,
            };

            // Migrar Windows → Instances
            v2.Instances = v1.Windows.Select(w => new InstanceSettingsV2
            {
                ProcessId       = w.ProcessId,
                Title           = w.Title,
                IsEnabled       = w.IsEnabled,
                DisplayOrder    = w.DisplayOrder,
                HotkeyModifiers = w.HotkeyModifiers,
                HotkeyKey       = w.HotkeyKey,
            }).ToList();

            return v2;
        }

        private static SettingsV2 DeserializeV2(JsonDocument doc)
        {
            return JsonSerializer.Deserialize<SettingsV2>(doc.RootElement.GetRawText())
                   ?? new SettingsV2();
        }
    }
}
