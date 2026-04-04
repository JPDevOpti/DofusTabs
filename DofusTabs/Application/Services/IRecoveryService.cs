namespace DofusTabs.Application.Services
{
    /// <summary>
    /// Recupera ventanas del juego a su estado original (desembebidas, con estilo correcto).
    /// Siempre seguro de llamar: en startup, shutdown y unhandled exceptions.
    /// </summary>
    public interface IRecoveryService
    {
        void RecoverAll(string reason);
    }
}
