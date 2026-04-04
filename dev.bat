@echo off
title DofusTabs - Modo Desarrollo
color 0A

echo ================================================
echo   DofusTabs - Desarrollo con Hot Reload
echo ================================================
echo.
echo Iniciando con dotnet watch...
echo Los cambios en codigo se aplicaran automaticamente.
echo Presiona Ctrl+C para detener.
echo.

cd /d "%~dp0DofusTabs"
dotnet watch run --project DofusTabs.csproj
