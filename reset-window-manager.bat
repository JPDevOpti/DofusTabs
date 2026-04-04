@echo off
setlocal EnableExtensions

title DofusTabs - Reinicio del manejador
color 0C

set "ROOT=%~dp0"
set "APP_DIR=%ROOT%DofusTabs"
set "APPDATA_DIR=%APPDATA%\DofusTabs"

echo ================================================
echo   DofusTabs - Reinicio y limpieza completa
echo ================================================
echo.
echo [1/5] Cerrando procesos de DofusTabs...
taskkill /IM DofusTabs.exe /F >nul 2>nul

echo [2/5] Cerrando procesos dotnet watch del proyecto...
for /f %%P in ('powershell -NoProfile -Command "Get-CimInstance Win32_Process ^| Where-Object { $_.Name -eq 'dotnet.exe' -and $_.CommandLine -like '*DofusTabs.csproj*' } ^| Select-Object -ExpandProperty ProcessId"') do (
    taskkill /PID %%P /F >nul 2>nul
)

echo [3/5] Limpiando build y temporales...
pushd "%ROOT%"
dotnet clean DofusTabs.sln -c Debug -v quiet >nul 2>nul
if exist "%APP_DIR%\bin" rmdir /S /Q "%APP_DIR%\bin"
if exist "%APP_DIR%\obj" rmdir /S /Q "%APP_DIR%\obj"
if exist "%APPDATA_DIR%\settings.json.tmp" del /F /Q "%APPDATA_DIR%\settings.json.tmp"
if exist "%APPDATA_DIR%\settings.json.bak" del /F /Q "%APPDATA_DIR%\settings.json.bak"
popd

echo [4/5] Preparando reinicio del manejador...
if /I "%~1"=="--no-restart" goto END

echo [5/5] Iniciando dotnet watch...
start "DofusTabs Watch" cmd /k "cd /d "%APP_DIR%" && dotnet watch run --project DofusTabs.csproj"

echo.
echo Reinicio completado. Se abrio una nueva consola con dotnet watch.
goto END

:END
echo.
echo Proceso finalizado.
endlocal
