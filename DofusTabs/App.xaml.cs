using System;
using System.Windows;
using System.Windows.Threading;
using DofusTabs.Core;

namespace DofusTabs
{
    public partial class App : Application
    {
        protected override void OnStartup(StartupEventArgs e)
        {
            WindowEmbeddingService.RecoverAllEmbeddedWindows();

            DispatcherUnhandledException += App_DispatcherUnhandledException;
            AppDomain.CurrentDomain.UnhandledException += CurrentDomain_UnhandledException;

            base.OnStartup(e);
        }

        protected override void OnExit(ExitEventArgs e)
        {
            WindowEmbeddingService.RecoverAllEmbeddedWindows();
            base.OnExit(e);
        }

        private void App_DispatcherUnhandledException(object sender, DispatcherUnhandledExceptionEventArgs e)
        {
            WindowEmbeddingService.RecoverAllEmbeddedWindows();
        }

        private void CurrentDomain_UnhandledException(object sender, UnhandledExceptionEventArgs e)
        {
            WindowEmbeddingService.RecoverAllEmbeddedWindows();
        }
    }
}

