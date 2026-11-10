@echo off
rem Double-click wrapper: runs bootstrap.ps1 with Windows PowerShell 5.1 and keeps the window open.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0bootstrap.ps1" %*
echo.
pause
