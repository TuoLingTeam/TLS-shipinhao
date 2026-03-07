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
set "CXFREEZE_BIN=%VENV_DIR%\Scripts\cxfreeze.exe"
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

if exist "%CXFREEZE_BIN%" (
  set "FREEZE_CMD=%CXFREEZE_BIN%"
) else (
  echo Installing cx_Freeze if needed...
  "%PYTHON_BIN%" -m pip install cx_Freeze -q
  if errorlevel 1 goto :error
  if exist "%VENV_DIR%\Scripts\cxfreeze.exe" (
    set "FREEZE_CMD=%VENV_DIR%\Scripts\cxfreeze.exe"
  ) else (
    set "FREEZE_CMD=cxfreeze"
  )
)

echo Building Windows package...
"%FREEZE_CMD%" ^
  --script main.py ^
  --target-dir "%APP_DIR%" ^
  --target-name "%APP_NAME%.exe" ^
  --base-name gui

if errorlevel 1 goto :error

if exist "%BUILD_DIR%" rmdir /s /q "%BUILD_DIR%"
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
