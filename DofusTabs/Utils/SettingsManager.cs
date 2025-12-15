using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using DofusTabs.Core;

namespace DofusTabs.Utils
{
    public class SettingsManager
    {
        private static string SettingsPath => Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "DofusTabs",
            "settings.json");

        public class WindowSettings
        {
            public string Title { get; set; } = string.Empty;
            public uint ProcessId { get; set; }
            public bool IsEnabled { get; set; } = true;
            public string IndividualHotkey { get; set; } = string.Empty;
            public int DisplayOrder { get; set; }
            public string HotkeyModifiers { get; set; } = string.Empty;
            public string HotkeyKey { get; set; } = string.Empty;
        }

        public class AppSettings
        {
            public string NextHotkeyModifiers { get; set; } = "Alt";
            public string NextHotkeyKey { get; set; } = "Tab";
            public string PreviousHotkeyModifiers { get; set; } = "Alt,Shift";
            public string PreviousHotkeyKey { get; set; } = "Tab";
            public List<WindowSettings> Windows { get; set; } = new List<WindowSettings>();
        }

        public static void SaveSettings(List<WindowInfo> windows, HotkeyManager? hotkeyManager)
        {
            try
            {
                var settings = new AppSettings();

                if (hotkeyManager != null)
                {
                    var nextHotkey = hotkeyManager.GetNextHotkeyConfig();
                    var previousHotkey = hotkeyManager.GetPreviousHotkeyConfig();
                    
                    settings.NextHotkeyModifiers = string.Join(",", GetModifierKeysString(nextHotkey.Modifiers));
                    settings.NextHotkeyKey = nextHotkey.Key.ToString();
                    settings.PreviousHotkeyModifiers = string.Join(",", GetModifierKeysString(previousHotkey.Modifiers));
                    settings.PreviousHotkeyKey = previousHotkey.Key.ToString();
                }

                foreach (var window in windows)
                {
                    var windowSettings = new WindowSettings
                    {
                        Title = window.Title,
                        ProcessId = window.ProcessId,
                        IsEnabled = window.IsEnabled,
                        IndividualHotkey = window.IndividualHotkey,
                        DisplayOrder = window.DisplayOrder
                    };

                    if (!string.IsNullOrEmpty(window.IndividualHotkey))
                    {
                        var hotkeyParts = ParseHotkeyString(window.IndividualHotkey);
                        windowSettings.HotkeyModifiers = string.Join(",", hotkeyParts.modifiers);
                        windowSettings.HotkeyKey = hotkeyParts.key;
                    }

                    settings.Windows.Add(windowSettings);
                }

                var directory = Path.GetDirectoryName(SettingsPath);
                if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
                {
                    Directory.CreateDirectory(directory);
                }

                var json = JsonSerializer.Serialize(settings, new JsonSerializerOptions { WriteIndented = true });
                File.WriteAllText(SettingsPath, json);
            }
            catch
            {
                // Ignorar errores al guardar
            }
        }

        public static AppSettings? LoadSettings()
        {
            try
            {
                if (File.Exists(SettingsPath))
                {
                    var json = File.ReadAllText(SettingsPath);
                    return JsonSerializer.Deserialize<AppSettings>(json);
                }
            }
            catch
            {
                // Ignorar errores al cargar
            }
            return null;
        }

        private static List<string> GetModifierKeysString(System.Windows.Input.ModifierKeys modifiers)
        {
            var result = new List<string>();
            if ((modifiers & System.Windows.Input.ModifierKeys.Control) != 0) result.Add("Control");
            if ((modifiers & System.Windows.Input.ModifierKeys.Alt) != 0) result.Add("Alt");
            if ((modifiers & System.Windows.Input.ModifierKeys.Shift) != 0) result.Add("Shift");
            return result;
        }

        private static (List<string> modifiers, string key) ParseHotkeyString(string hotkeyString)
        {
            var parts = hotkeyString.Split(new[] { " + " }, StringSplitOptions.RemoveEmptyEntries);
            var modifiers = new List<string>();
            string key = "";

            for (int i = 0; i < parts.Length - 1; i++)
            {
                var part = parts[i].Trim();
                if (part == "Ctrl" || part == "Control")
                    modifiers.Add("Control");
                else if (part == "Alt")
                    modifiers.Add("Alt");
                else if (part == "Shift")
                    modifiers.Add("Shift");
            }

            if (parts.Length > 0)
            {
                key = parts[parts.Length - 1].Trim();
            }

            return (modifiers, key);
        }

        public static System.Windows.Input.ModifierKeys ParseModifiers(string modifiersString)
        {
            var modifiers = System.Windows.Input.ModifierKeys.None;
            var parts = modifiersString.Split(new[] { ',' }, StringSplitOptions.RemoveEmptyEntries);
            
            foreach (var part in parts)
            {
                var trimmed = part.Trim();
                if (trimmed == "Control" || trimmed == "Ctrl")
                    modifiers |= System.Windows.Input.ModifierKeys.Control;
                else if (trimmed == "Alt")
                    modifiers |= System.Windows.Input.ModifierKeys.Alt;
                else if (trimmed == "Shift")
                    modifiers |= System.Windows.Input.ModifierKeys.Shift;
            }

            return modifiers;
        }
    }
}

