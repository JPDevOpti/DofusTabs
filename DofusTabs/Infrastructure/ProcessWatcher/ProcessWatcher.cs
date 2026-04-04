using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Windows.Threading;
using DofusTabs.Application.Services;
using DofusTabs.Diagnostics;
using DofusTabs.Infrastructure.Embedding;

namespace DofusTabs.Infrastructure.ProcessWatcher
{
    /// <summary>
    /// Polling ligero (1s) sobre nombres de proceso del juego.
    /// Dispara ProcessAppeared/ProcessDisappeared en el hilo del dispatcher.
    /// No llama a EnumWindows — solo Process.GetProcessesByName, que es muy barato.
    /// </summary>
    public sealed class ProcessWatcher : IProcessWatcher
    {
        private readonly DispatcherTimer _timer;
        private HashSet<int> _lastKnownPids = new();

        public event EventHandler<uint>? ProcessAppeared;
        public event EventHandler<uint>? ProcessDisappeared;

        public ProcessWatcher()
        {
            _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
            _timer.Tick += OnTick;
        }

        public void Start() => _timer.Start();
        public void Stop()  => _timer.Stop();

        public void Dispose() => _timer.Stop();

        private void OnTick(object? sender, EventArgs e)
        {
            var current = GetGamePids();

            foreach (var pid in current.Except(_lastKnownPids))
            {
                AppLogger.Info($"ProcessWatcher: proceso aparecido pid={pid}");
                ProcessAppeared?.Invoke(this, (uint)pid);
            }

            foreach (var pid in _lastKnownPids.Except(current))
            {
                AppLogger.Info($"ProcessWatcher: proceso desaparecido pid={pid}");
                ProcessDisappeared?.Invoke(this, (uint)pid);
            }

            _lastKnownPids = current;
        }

        private static HashSet<int> GetGamePids()
        {
            var result = new HashSet<int>();
            foreach (var name in EmbeddingRegistry.KnownProcessNames)
            {
                try
                {
                    foreach (var p in Process.GetProcessesByName(name))
                        result.Add(p.Id);
                }
                catch { /* proceso ya no existe */ }
            }
            return result;
        }
    }
}
