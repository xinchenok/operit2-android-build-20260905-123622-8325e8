@echo off
where pwsh.exe >nul 2>nul
if errorlevel 1 (
  echo PowerShell 7 is required. Nothing was installed.
  pause
  exit /b 1
)
pwsh.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0Update-Operit2.ps1"
set "RC=%ERRORLEVEL%"
pause
exit /b %RC%
