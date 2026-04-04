@echo off
setlocal EnableExtensions

cd /d "%~dp0"

echo ================================================
echo   DofusTabs - Watch limpio
echo ================================================
echo.

echo [1/3] Cerrando procesos anteriores...
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-Process -Name DofusTabs -ErrorAction SilentlyContinue | Stop-Process -Force; Get-CimInstance Win32_Process -Filter \"Name='dotnet.exe'\" | Where-Object { $_.CommandLine -match 'DofusTabs.csproj|dotnet-watch|DofusTabs.sln' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"

echo [2/3] Build inicial...
dotnet build "DofusTabs.sln" -c Debug -v minimal
if errorlevel 1 (
    echo.
    echo [ERROR] El build inicial fallo.
    exit /b 1
)

echo [3/3] Iniciando dotnet watch...
dotnet watch --project "DofusTabs/DofusTabs.csproj" run

exit /b %ERRORLEVEL%
