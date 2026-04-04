using DofusTabs.Application.Settings;

namespace DofusTabs.Application.Services
{
    public interface ISettingsService
    {
        AppSettings Load();
        void Save(AppSettings settings);
    }
}
