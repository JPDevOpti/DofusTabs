using System;
using System.Collections.Generic;
using System.Linq;
using DofusTabs.Application.Services;
using DofusTabs.Domain;
using DofusTabs.Infrastructure.Embedding;
using DofusTabs.Infrastructure.Win32;

namespace DofusTabs.Infrastructure.Discovery
{
    public sealed class GameDiscoveryService : IGameDiscoveryService
    {
        private Dictionary<uint, GameInstance> _previousInstances = new();
        private Dictionary<uint, IntPtr> _handleMap = new();

        public IReadOnlyList<GameInstance> GetSnapshot(bool preserveState = true)
        {
            var rawWindows = WindowEnumerator.EnumerateGameWindows(EmbeddingRegistry.KnownProcessNames);
            var bestWindows = rawWindows
                .GroupBy(w => w.ProcessId)
                .Select(g => g
                    .OrderByDescending(w => ScoreTitle(w.Title))
                    .ThenByDescending(w => w.Title.Length)
                    .First())
                .ToList();

            int nextOrder = _previousInstances.Any()
                ? _previousInstances.Values.Max(i => i.DisplayOrder) + 1
                : 0;

            var newInstances = new Dictionary<uint, GameInstance>(bestWindows.Count);
            var newHandleMap = new Dictionary<uint, IntPtr>(bestWindows.Count);

            foreach (var raw in bestWindows)
            {
                var instance = GameInstance.FromWindowData(raw.ProcessId, raw.Title, raw.ProcessName);

                if (preserveState && _previousInstances.TryGetValue(raw.ProcessId, out var existing))
                {
                    instance.IsEnabled        = existing.IsEnabled;
                    instance.DisplayOrder     = existing.DisplayOrder;
                    instance.IndividualHotkey = existing.IndividualHotkey;
                    instance.IsActive         = existing.IsActive;
                }
                else
                {
                    instance.DisplayOrder = nextOrder++;
                }

                newInstances[raw.ProcessId] = instance;
                newHandleMap[raw.ProcessId] = raw.Handle;
            }

            _previousInstances = newInstances;
            _handleMap = newHandleMap;

            return _previousInstances.Values
                .OrderBy(i => i.DisplayOrder)
                .ThenBy(i => i.CharacterName)
                .ToList();
        }

        public bool TryGetWindowHandle(uint processId, out IntPtr handle) =>
            _handleMap.TryGetValue(processId, out handle);

        public void ApplyPersistedSettings(
            IReadOnlyList<Application.Settings.InstanceSettings> saved)
        {
            foreach (var s in saved)
            {
                if (_previousInstances.TryGetValue(s.ProcessId, out var instance))
                {
                    instance.DisplayOrder     = s.DisplayOrder;
                    instance.IsEnabled        = s.IsEnabled;
                    instance.IndividualHotkey = s.IndividualHotkey;
                }
            }
        }

        private static int ScoreTitle(string title)
        {
            if (string.IsNullOrWhiteSpace(title)) return int.MinValue;

            string trimmed = title.Trim();
            if (IsNoiseTitle(trimmed)) return int.MinValue + 100;

            int score = Math.Min(trimmed.Length, 120);
            if (trimmed.Contains(" - ", StringComparison.Ordinal))    score += 180;
            if (trimmed.Contains("|",   StringComparison.Ordinal))    score += 60;
            if (trimmed.Contains("dofus", StringComparison.OrdinalIgnoreCase)) score += 20;
            return score;
        }

        private static bool IsNoiseTitle(string title) =>
            title.Equals("MSCTFIME UI",  StringComparison.OrdinalIgnoreCase)
            || title.Equals("Default IME", StringComparison.OrdinalIgnoreCase)
            || title.Equals("Release",     StringComparison.OrdinalIgnoreCase);
    }
}
