@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-ffmpeg.ps1" %*
echo.
pause
