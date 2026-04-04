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
        // Estado interno preservado entre snapshots
        private Dictionary<uint, GameInstance> _previousInstances = new();
        private Dictionary<uint, IntPtr> _handleMap = new();

        public IReadOnlyList<GameInstance> GetSnapshot(bool preserveState = true)
        {
            var rawWindows = WindowEnumerator.EnumerateGameWindows(EmbeddingRegistry.KnownProcessNames);

            // Calcular DisplayOrder máximo del ciclo anterior para nuevas instancias
            int nextOrder = _previousInstances.Any()
                ? _previousInstances.Values.Max(i => i.DisplayOrder) + 1
                : 0;

            var newInstances = new Dictionary<uint, GameInstance>();
            var newHandleMap = new Dictionary<uint, IntPtr>();

            foreach (var raw in rawWindows)
            {
                // Si ya tenemos este processId en este ciclo, ignorar (múltiples ventanas del mismo proceso)
                if (newInstances.ContainsKey(raw.ProcessId))
                    continue;

                GameInstance instance;
                if (preserveState && _previousInstances.TryGetValue(raw.ProcessId, out var existing))
                {
                    // Ventana conocida: preservar estado mutable
                    instance = GameInstance.FromWindowData(raw.ProcessId, raw.Title, raw.ProcessName);
                    instance.IsEnabled      = existing.IsEnabled;
                    instance.DisplayOrder   = existing.DisplayOrder;
                    instance.IndividualHotkey = existing.IndividualHotkey;
                    instance.IsActive       = existing.IsActive;
                }
                else
                {
                    instance = GameInstance.FromWindowData(raw.ProcessId, raw.Title, raw.ProcessName);
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

        /// <summary>
        /// Aplica configuración guardada (DisplayOrder, IsEnabled, IndividualHotkey) a las instancias actuales.
        /// Llamar una vez en startup tras el primer GetSnapshot.
        /// </summary>
        public void ApplyPersistedSettings(
            IReadOnlyList<Application.Settings.InstanceSettings> saved)
        {
            foreach (var s in saved)
            {
                if (_previousInstances.TryGetValue(s.ProcessId, out var instance))
                {
                    instance.DisplayOrder   = s.DisplayOrder;
                    instance.IsEnabled      = s.IsEnabled;
                    instance.IndividualHotkey = s.IndividualHotkey;
                }
            }
        }
    }
}
