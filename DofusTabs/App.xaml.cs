using System;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Threading;
using DofusTabs.Application.Services;
using DofusTabs.Diagnostics;
using DofusTabs.Domain;
using DofusTabs.Infrastructure.Embedding;
using DofusTabs.Infrastructure.Discovery;
using DofusTabs.Infrastructure.Hotkeys;
using DofusTabs.Infrastructure.Settings;
using DofusTabs.UI;
using Microsoft.Extensions.DependencyInjection;
using ProcessWatcherImpl = DofusTabs.Infrastructure.ProcessWatcher.ProcessWatcher;

namespace DofusTabs
{
    public partial class App : System.Windows.Application
    {
        private ServiceProvider? _services;

        protected override void OnStartup(StartupEventArgs e)
        {
            // Recovery de emergencia ANTES de que el container esté listo:
            // recupera ventanas del juego embebidas en una sesión anterior que terminó mal.
            RecoveryService.EnsureCleanState();

            AppLogger.Info("=== DofusTabs iniciando ===");

            // Validación fail-fast: detecta aliases duplicados en GameClass.All antes de arrancar
            GameClass.ValidateNoDuplicateAliases();

            // Handlers de errores no controlados
            DispatcherUnhandledException           += OnDispatcherUnhandledException;
            AppDomain.CurrentDomain.UnhandledException += OnDomainUnhandledException;
            TaskScheduler.UnobservedTaskException  += OnUnobservedTaskException;

            // Construir DI container
            _services = BuildServices();

            base.OnStartup(e);

            // Crear y mostrar ventana principal con dependencias inyectadas
            var mainWindow = _services.GetRequiredService<MainWindow>();
            MainWindow = mainWindow;
            mainWindow.Show();

            AppLogger.Info("MainWindow mostrada");
        }

        protected override void OnExit(ExitEventArgs e)
        {
            AppLogger.Info("DofusTabs cerrando");
            try
            {
                _services?.GetRequiredService<IRecoveryService>().RecoverAll("shutdown normal");
            }
            catch { /* nunca bloquear el cierre */ }

            _services?.Dispose();
            base.OnExit(e);
        }

        // ── Handlers de errores ──────────────────────────────────────────────

        private void OnDispatcherUnhandledException(object sender, DispatcherUnhandledExceptionEventArgs e)
        {
            AppLogger.Error(e.Exception, "Excepción no controlada en Dispatcher");
            RunEmergencyRecovery("DispatcherUnhandledException");
        }

        private void OnDomainUnhandledException(object sender, UnhandledExceptionEventArgs e)
        {
            if (e.ExceptionObject is Exception ex)
                AppLogger.Error(ex, "Excepción no controlada en AppDomain");
            RunEmergencyRecovery("DomainUnhandledException");
        }

        private void OnUnobservedTaskException(object? sender, UnobservedTaskExceptionEventArgs e)
        {
            AppLogger.Error(e.Exception, "Excepción no observada en Task");
        }

        private void RunEmergencyRecovery(string source)
        {
            try
            {
                _services?.GetRequiredService<IRecoveryService>().RecoverAll(source);
            }
            catch
            {
                RecoveryService.EnsureCleanState();
            }
        }

        // ── Composition Root ─────────────────────────────────────────────────

        private static ServiceProvider BuildServices()
        {
            var services = new ServiceCollection();

            // Infrastructure — Embedding
            services.AddSingleton<EmbeddingRegistry>();
            services.AddSingleton<IEmbeddingRegistry>(sp => sp.GetRequiredService<EmbeddingRegistry>());
            services.AddSingleton<IEmbeddingService>(sp =>
                new EmbeddingService(sp.GetRequiredService<IEmbeddingRegistry>()));
            services.AddSingleton<IRecoveryService>(sp =>
                new RecoveryService(sp.GetRequiredService<IEmbeddingService>()));

            // Infrastructure — Discovery
            services.AddSingleton<GameDiscoveryService>();
            services.AddSingleton<IGameDiscoveryService>(sp =>
                sp.GetRequiredService<GameDiscoveryService>());

            // Infrastructure — Hotkeys
            services.AddSingleton<IHotkeyService, HotkeyService>();

            // Infrastructure — ProcessWatcher
            services.AddSingleton<IProcessWatcher, ProcessWatcherImpl>();

            // Infrastructure — Settings
            services.AddSingleton<ISettingsService, SettingsService>();

            // UI
            services.AddTransient<MainWindow>();

            return services.BuildServiceProvider();
        }
    }
}
