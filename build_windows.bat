@echo off
setlocal

cd /d "%~dp0"

set "PYTHON_BIN=%CD%\.venv\Scripts\python.exe"
if not exist "%PYTHON_BIN%" (
  set "PYTHON_BIN=python"
)

"%PYTHON_BIN%" scripts\build.py windows
exit /b %errorlevel%
