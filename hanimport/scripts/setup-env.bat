@echo off
setlocal EnableExtensions
chcp 65001 >nul

set "ROOT=%~dp0.."
set "REPO=%~dp0..\.."
cd /d "%REPO%"

echo ========================================
echo   小寒导入器 — 环境安装
echo ========================================
echo.

set "PYTHON="
where python >nul 2>&1 && set "PYTHON=python"
if not defined PYTHON (
  where py >nul 2>&1 && set "PYTHON=py -3"
)
if not defined PYTHON (
  echo [错误] 未找到 Python 3.10+
  echo 下载: https://www.python.org/downloads/
  pause
  exit /b 1
)

echo [1/3] Python: 
%PYTHON% --version

echo.
echo [2/3] 安装 UnityPy 依赖 …
%PYTHON% -m pip install -r "%ROOT%\scripts\requirements.txt"
if errorlevel 1 (
  echo [错误] pip 安装失败
  pause
  exit /b 1
)

echo.
echo [3/3] 编译 hanimport release …
cargo build --release -p hanimport
if errorlevel 1 (
  echo [警告] release 编译失败，仍可使用网页版（Python 解包）
) else (
  echo [ok] exe: hanpet\src-tauri\target\release\hanimport.exe
)

echo.
echo 安装完成。可双击「启动小寒导入器.bat」使用。
pause
exit /b 0
