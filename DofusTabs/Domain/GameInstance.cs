using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;

namespace DofusTabs.Domain
{
    public sealed class GameInstance : INotifyPropertyChanged
    {
        private static readonly string[] IgnoredTitleTokens = { "dofus", "touch", "ankama", "launcher" };

        private bool _isEnabled = true;
        private bool _isActive;
        private bool _isPrimary;
        private int _displayOrder;
        private HotkeyBinding? _individualHotkey;

        public uint ProcessId { get; }
        public string WindowTitle { get; }
        public string ProcessName { get; }
        public string CharacterName { get; }
        public GameClass? Class { get; }

        public string CharacterClass => Class?.CanonicalName ?? string.Empty;

        public string AvatarInitial => string.IsNullOrWhiteSpace(CharacterName)
            ? "?"
            : CharacterName.Substring(0, 1).ToUpperInvariant();

        public string DisplayTitle
        {
            get
            {
                if (string.IsNullOrEmpty(WindowTitle)) return WindowTitle;
                var parts = WindowTitle.Split(new[] { " - " }, StringSplitOptions.RemoveEmptyEntries);
                return parts.Length >= 2 ? $"{parts[1]} - {parts[0]}" : WindowTitle;
            }
        }

        public string SidebarCharacterName => string.IsNullOrWhiteSpace(CharacterName)
            ? DisplayTitle
            : CharacterName;

        public string IconPath { get; }

        public bool IsEnabled
        {
            get => _isEnabled;
            set { if (_isEnabled != value) { _isEnabled = value; OnPropertyChanged(nameof(IsEnabled)); } }
        }

        public int DisplayOrder
        {
            get => _displayOrder;
            set { if (_displayOrder != value) { _displayOrder = value; OnPropertyChanged(nameof(DisplayOrder)); } }
        }

        public HotkeyBinding? IndividualHotkey
        {
            get => _individualHotkey;
            set { if (_individualHotkey != value) { _individualHotkey = value; OnPropertyChanged(nameof(IndividualHotkey)); } }
        }

        public bool IsActive
        {
            get => _isActive;
            set { if (_isActive != value) { _isActive = value; OnPropertyChanged(nameof(IsActive)); } }
        }

        public bool IsPrimary
        {
            get => _isPrimary;
            set { if (_isPrimary != value) { _isPrimary = value; OnPropertyChanged(nameof(IsPrimary)); } }
        }

        private GameInstance(uint processId, string windowTitle, string processName,
            string characterName, GameClass? cls)
        {
            ProcessId = processId;
            WindowTitle = windowTitle;
            ProcessName = processName;
            CharacterName = characterName;
            Class = cls;
            IconPath = ResolveIconPath(cls);
        }

        public static GameInstance FromWindowData(uint processId, string title, string processName)
        {
            var (name, cls) = ParseTitle(title);
            return new GameInstance(processId, title, processName, name, cls);
        }

        private static string ResolveIconPath(GameClass? cls)
        {
            if (cls == null) return string.Empty;
            var fileName = cls.IconFile;
            var appDir = AppContext.BaseDirectory;
            var cwd = Directory.GetCurrentDirectory();
            var candidates = new[]
            {
                Path.Combine("Resources", "Icons", fileName),
                Path.Combine(appDir, "Resources", "Icons", fileName),
                Path.Combine(cwd, "Resources", "Icons", fileName),
                Path.Combine(cwd, "DofusTabs", "Resources", "Icons", fileName),
            };
            foreach (var p in candidates)
                if (File.Exists(p)) return Path.GetFullPath(p);
            return string.Empty;
        }

        private static (string name, GameClass? cls) ParseTitle(string title)
        {
            if (string.IsNullOrWhiteSpace(title)) return (string.Empty, null);

            // Split only on title separators like " - " / " | " / " – " / " — ",
            // preserving hyphens inside character names (e.g. "Juan-Perez").
            var tokens = Regex.Split(title, @"\s+(?:-|\||–|—)\s+")
                .Select(t => t.Trim())
                .Where(t => !string.IsNullOrWhiteSpace(t))
                .ToList();

            GameClass? resolved = null;
            foreach (var token in tokens)
            {
                resolved = GameClass.ResolveFromText(token);
                if (resolved != null) break;
            }
            if (resolved == null)
                resolved = GameClass.ResolveFromText(title);

            string name = tokens.FirstOrDefault(t => IsValidNameToken(t, resolved)) ?? string.Empty;
            if (string.IsNullOrWhiteSpace(name) && tokens.Count > 0)
                name = tokens[0];

            return (name, resolved);
        }

        private static bool IsValidNameToken(string token, GameClass? cls)
        {
            if (string.IsNullOrWhiteSpace(token)) return false;
            string normalized = GameClass.NormalizeText(token);
            foreach (var ignored in IgnoredTitleTokens)
                if (normalized.Contains(ignored, StringComparison.OrdinalIgnoreCase)) return false;
            if (cls != null && GameClass.ResolveFromText(token) == cls) return false;
            return true;
        }

        public event PropertyChangedEventHandler? PropertyChanged;
        private void OnPropertyChanged(string propertyName) =>
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
