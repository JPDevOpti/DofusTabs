using System;

namespace DofusTabs.Application.Services
{
    public interface IProcessWatcher : IDisposable
    {
        void Start();
        void Stop();

        event EventHandler<uint>? ProcessAppeared;

        event EventHandler<uint>? ProcessDisappeared;
    }
}
