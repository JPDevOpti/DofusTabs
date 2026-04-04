@echo off
setlocal EnableExtensions

cd /d "%~dp0"

if exist "%~dp0watch-clean-run.bat" (
	call "%~dp0watch-clean-run.bat"
	set "exitCode=%ERRORLEVEL%"
) else (
	echo [dev] watch-clean-run.bat no encontrado. Iniciando dotnet watch directo...
	dotnet watch --project "DofusTabs/DofusTabs.csproj" run
	set "exitCode=%ERRORLEVEL%"
)

if not "%exitCode%"=="0" (
	echo.
	echo [dev] El entorno de desarrollo finalizo con codigo %exitCode%.
	echo [dev] Presiona una tecla para cerrar esta ventana...
	pause >nul
)

exit /b %exitCode%
