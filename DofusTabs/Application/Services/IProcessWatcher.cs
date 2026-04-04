using System;

namespace DofusTabs.Application.Services
{
    public interface IProcessWatcher : IDisposable
    {
        void Start();
        void Stop();

        /// <summary>Dispara en el hilo del dispatcher cuando aparece un proceso del juego nuevo.</summary>
        event EventHandler<uint>? ProcessAppeared;

        /// <summary>Dispara en el hilo del dispatcher cuando desaparece un proceso del juego.</summary>
        event EventHandler<uint>? ProcessDisappeared;
    }
}
