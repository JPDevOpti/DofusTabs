using System;
using System.IO;

namespace DofusTabs.Diagnostics
{
    public static class AppLogger
    {
        private static readonly string LogDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "DofusTabs", "logs");

        private static readonly object _lock = new();

        private static string LogPath =>
            Path.Combine(LogDir, $"app-{DateTime.Today:yyyy-MM-dd}.log");

        public static void Info(string message)  => Write("INFO",  message);
        public static void Warn(string message)  => Write("WARN",  message);
        public static void Error(Exception ex, string message) =>
            Write("ERROR", $"{message}{Environment.NewLine}  {ex.GetType().Name}: {ex.Message}{Environment.NewLine}{ex.StackTrace}");

        private static void Write(string level, string message)
        {
            try
            {
                lock (_lock)
                {
                    Directory.CreateDirectory(LogDir);
                    File.AppendAllText(
                        LogPath,
                        $"{DateTime.Now:HH:mm:ss.fff} [{level}] {message}{Environment.NewLine}");
                }
            }
            catch { }
        }
    }
}
