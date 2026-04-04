using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Windows.Input;
using DofusTabs.Application.Services;
using DofusTabs.Application.Settings;
using DofusTabs.Diagnostics;
using DofusTabs.Domain;

namespace DofusTabs.Infrastructure.Settings
{
    public sealed class SettingsService : ISettingsService
    {
        private static string SettingsPath => Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "DofusTabs", "settings.json");

        public AppSettings Load()
        {
            try
            {
                if (!File.Exists(SettingsPath))
                    return new AppSettings();

                var json = File.ReadAllText(SettingsPath);
                using var doc = JsonDocument.Parse(json);
                var v2 = SettingsMigrator.MigrateToLatest(doc);
                return ToAppSettings(v2);
            }
            catch (Exception ex)
            {
                AppLogger.Error(ex, "SettingsService: error cargando settings, usando valores por defecto");
                return new AppSettings();
            }
        }

        public void Save(AppSettings settings)
        {
            try
            {
                var v2 = ToV2(settings);
                var json = JsonSerializer.Serialize(v2, new JsonSerializerOptions { WriteIndented = true });

                var dir = Path.GetDirectoryName(SettingsPath)!;
                if (!Directory.Exists(dir)) Directory.CreateDirectory(dir);

                var tmpPath = SettingsPath + ".tmp";
                var bakPath = SettingsPath + ".bak";

                File.WriteAllText(tmpPath, json);

                if (File.Exists(SettingsPath))
                    File.Replace(tmpPath, SettingsPath, bakPath, ignoreMetadataErrors: true);
                else
                    File.Move(tmpPath, SettingsPath);
            }
            catch (Exception ex)
            {
                AppLogger.Error(ex, "SettingsService: error guardando settings");
            }
        }

        // ── Conversiones V2 ↔ AppSettings ───────────────────────────────────

        private static AppSettings ToAppSettings(SettingsV2 v2)
        {
            return new AppSettings
            {
                NextHotkey     = SanitizeGlobalBinding(
                    ParseBinding(v2.NextHotkeyModifiers, v2.NextHotkeyKey),
                    new HotkeyBinding(ModifierKeys.Alt, Key.Tab)),
                PreviousHotkey = SanitizeGlobalBinding(
                    ParseBinding(v2.PreviousHotkeyModifiers, v2.PreviousHotkeyKey),
                    new HotkeyBinding(ModifierKeys.Alt | ModifierKeys.Shift, Key.Tab)),
                Instances      = MapInstances(v2.Instances),
            };
        }

        private static List<InstanceSettings> MapInstances(List<InstanceSettingsV2> raw)
        {
            var result = new List<InstanceSettings>(raw.Count);
            foreach (var r in raw)
            {
                HotkeyBinding? individual = null;
                if (!string.IsNullOrWhiteSpace(r.HotkeyKey))
                {
                    var parsed = ParseBinding(r.HotkeyModifiers, r.HotkeyKey);
                    if (!parsed.IsEmpty && HasSupportedModifier(parsed.Modifiers))
                        individual = parsed;
                }

                result.Add(new InstanceSettings
                {
                    ProcessId       = r.ProcessId,
                    Title           = r.Title,
                    IsEnabled       = r.IsEnabled,
                    DisplayOrder    = r.DisplayOrder,
                    IndividualHotkey = individual,
                });
            }
            return result;
        }

        private static SettingsV2 ToV2(AppSettings s)
        {
            var v2 = new SettingsV2
            {
                NextHotkeyModifiers     = ModifiersToString(s.NextHotkey.Modifiers),
                NextHotkeyKey           = s.NextHotkey.Key.ToString(),
                PreviousHotkeyModifiers = ModifiersToString(s.PreviousHotkey.Modifiers),
                PreviousHotkeyKey       = s.PreviousHotkey.Key.ToString(),
            };

            foreach (var inst in s.Instances)
            {
                v2.Instances.Add(new InstanceSettingsV2
                {
                    ProcessId    = inst.ProcessId,
                    Title        = inst.Title,
                    IsEnabled    = inst.IsEnabled,
                    DisplayOrder = inst.DisplayOrder,
                    HotkeyModifiers = inst.IndividualHotkey != null
                        ? ModifiersToString(inst.IndividualHotkey.Modifiers) : string.Empty,
                    HotkeyKey = inst.IndividualHotkey != null && HasSupportedModifier(inst.IndividualHotkey.Modifiers)
                        ? inst.IndividualHotkey.Key.ToString()
                        : string.Empty,
                });
            }

            return v2;
        }

        // ── Helpers de serialización de hotkeys ─────────────────────────────

        private static HotkeyBinding ParseBinding(string modifiersStr, string keyStr)
        {
            var modifiers = ModifierKeys.None;
            foreach (var part in modifiersStr.Split(','))
            {
                var t = part.Trim();
                if (t is "Control" or "Ctrl") modifiers |= ModifierKeys.Control;
                else if (t == "Alt")   modifiers |= ModifierKeys.Alt;
                else if (t == "Shift") modifiers |= ModifierKeys.Shift;
            }

            if (!Enum.TryParse<Key>(keyStr, out var key)) key = Key.None;
            return new HotkeyBinding(modifiers, key);
        }

        private static string ModifiersToString(ModifierKeys modifiers)
        {
            var parts = new List<string>();
            if ((modifiers & ModifierKeys.Control) != 0) parts.Add("Control");
            if ((modifiers & ModifierKeys.Alt)     != 0) parts.Add("Alt");
            if ((modifiers & ModifierKeys.Shift)   != 0) parts.Add("Shift");
            return string.Join(",", parts);
        }

        private static HotkeyBinding SanitizeGlobalBinding(HotkeyBinding binding, HotkeyBinding fallback)
        {
            if (binding.IsEmpty || !HasSupportedModifier(binding.Modifiers))
                return fallback;
            return binding;
        }

        private static bool HasSupportedModifier(ModifierKeys modifiers) =>
            (modifiers & (ModifierKeys.Control | ModifierKeys.Alt | ModifierKeys.Shift)) != 0;
    }
}
