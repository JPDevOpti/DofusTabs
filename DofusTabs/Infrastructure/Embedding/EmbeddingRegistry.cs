using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using DofusTabs.Application.Services;

namespace DofusTabs.Infrastructure.Embedding
{
    /// <summary>
    /// Registro thread-safe de instancias embebidas actualmente activas.
    /// Fuente de verdad para RecoveryService: sabe exactamente qué restaurar.
    /// </summary>
    public sealed class EmbeddingRegistry : IEmbeddingRegistry
    {
        /// <summary>
        /// Nombres de proceso del juego conocidos. Fuente única para toda la app
        /// (RecoveryService, WindowEnumerator).
        /// </summary>
        public static readonly IReadOnlySet<string> KnownProcessNames =
            new HashSet<string>(StringComparer.OrdinalIgnoreCase) { "dofus", "dofustouch" };

        private readonly ConcurrentDictionary<uint, IntPtr> _hwndByPid = new();
        private readonly ConcurrentDictionary<uint, EmbeddedWindowSnapshot> _snapshotByPid = new();

        public void Register(uint processId, IntPtr hwnd)
        {
            _hwndByPid[processId] = hwnd;
        }

        internal void RegisterSnapshot(uint processId, EmbeddedWindowSnapshot snapshot)
        {
            _snapshotByPid[processId] = snapshot;
        }

        public void Unregister(uint processId)
        {
            _hwndByPid.TryRemove(processId, out _);
            _snapshotByPid.TryRemove(processId, out _);
        }

        public bool IsRegistered(uint processId) => _hwndByPid.ContainsKey(processId);

        public IReadOnlyList<(uint ProcessId, IntPtr Hwnd)> GetAll() =>
            _hwndByPid.Select(kvp => (kvp.Key, kvp.Value)).ToList();

        internal bool TryGetSnapshot(uint processId, out EmbeddedWindowSnapshot? snapshot) =>
            _snapshotByPid.TryGetValue(processId, out snapshot);
    }
}
