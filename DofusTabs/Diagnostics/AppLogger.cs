using System;
using System.IO;

namespace DofusTabs.Diagnostics
{
    /// <summary>
    /// Logger de archivo simple. Sin dependencias externas.
    /// Rotación diaria — conserva el archivo del día actual.
    /// Reemplazable con Serilog en el futuro cambiando solo esta clase.
    /// </summary>
    public static class AppLogger
    {
        private static readonly string LogDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "DofusTabs", "logs");

        private static readonly object _lock = new object();

        private static string LogPath =>
            Path.Combine(LogDir, $"app-{DateTime.Today:yyyy-MM-dd}.log");

        public static void Info(string message)  => Write("INFO",  message);
        public static void Warn(string message)  => Write("WARN",  message);
        public static void Error(Exception ex, string message) =>
            Write("ERROR", $"{message}\n  {ex.GetType().Name}: {ex.Message}");

        private static void Write(string level, string message)
        {
            try
            {
                lock (_lock)
                {
                    if (!Directory.Exists(LogDir))
                        Directory.CreateDirectory(LogDir);

                    File.AppendAllText(
                        LogPath,
                        $"{DateTime.Now:HH:mm:ss.fff} [{level}] {message}{Environment.NewLine}");
                }
            }
            catch
            {
                // El logger nunca debe lanzar excepciones
            }
        }
    }
}
