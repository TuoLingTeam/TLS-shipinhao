@echo off
setlocal

cd /d "%~dp0"

set "DIST_DIR=%CD%\dist"
set "BUILD_DIR=%CD%\build"
for %%I in ("%CD%") do set "APP_NAME=%%~nxI"
set "APP_DIR=%DIST_DIR%\%APP_NAME%"
set "EXE_FILE=%APP_DIR%\%APP_NAME%.exe"
set "OLD_EXE_FILE=%DIST_DIR%\%APP_NAME%.exe"
set "SPEC_FILE=%CD%\%APP_NAME%.spec"
set "VENV_DIR=%CD%\.venv"
set "PYTHON_BIN=%VENV_DIR%\Scripts\python.exe"
set "PYINSTALLER_BIN=%VENV_DIR%\Scripts\pyinstaller.exe"
set "COOKIE_FILE=%CD%\cookie.txt"
set "MAGIC_FILE=%CD%\biz_magic.txt"

if not exist "%PYTHON_BIN%" (
  set "PYTHON_BIN=python"
)

echo Cleaning old build artifacts...
if exist "%BUILD_DIR%" rmdir /s /q "%BUILD_DIR%"
if exist "%SPEC_FILE%" del /f /q "%SPEC_FILE%"
if exist "%OLD_EXE_FILE%" del /f /q "%OLD_EXE_FILE%"
if exist "%APP_DIR%" (
  if exist "%APP_DIR%\*" (
    rmdir /s /q "%APP_DIR%"
  ) else (
    del /f /q "%APP_DIR%"
  )
)
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"

echo Ensuring Qt build dependencies are installed...
"%PYTHON_BIN%" -c "import PySide6, requests, charset_normalizer" >nul 2>&1
if errorlevel 1 (
  "%PYTHON_BIN%" -m pip install PySide6 requests charset_normalizer -q
  if errorlevel 1 goto :error
)

if exist "%PYINSTALLER_BIN%" (
  set "PYINSTALLER_CMD=%PYINSTALLER_BIN%"
) else (
  echo Installing PyInstaller if needed...
  "%PYTHON_BIN%" -m pip install pyinstaller -q
  if errorlevel 1 goto :error
  if exist "%VENV_DIR%\Scripts\pyinstaller.exe" (
    set "PYINSTALLER_CMD=%VENV_DIR%\Scripts\pyinstaller.exe"
  ) else (
    set "PYINSTALLER_CMD=pyinstaller"
  )
)

echo Building Windows package...
"%PYINSTALLER_CMD%" ^
  --clean ^
  --noconfirm ^
  --onedir ^
  --windowed ^
  --collect-all charset_normalizer ^
  --collect-all shiboken6 ^
  --collect-all PySide6 ^
  --name "%APP_NAME%" ^
  main.py

if errorlevel 1 goto :error

if exist "%BUILD_DIR%" rmdir /s /q "%BUILD_DIR%"
if exist "%SPEC_FILE%" del /f /q "%SPEC_FILE%"
if exist "%COOKIE_FILE%" copy /y "%COOKIE_FILE%" "%APP_DIR%\cookie.txt" >nul
if exist "%MAGIC_FILE%" copy /y "%MAGIC_FILE%" "%APP_DIR%\biz_magic.txt" >nul

echo Build complete.
echo Output folder: %APP_DIR%
echo Executable: %EXE_FILE%
echo cookie.txt and biz_magic.txt will be copied automatically when they exist in the project root.
goto :eof

:error
echo Build failed.
exit /b 1
