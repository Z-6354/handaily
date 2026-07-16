@echo off
setlocal EnableExtensions
chcp 65001 >nul

rem ROOT = hanimport/; REPO = monorepo root (HANDAILY/)
set "ROOT=%~dp0"
set "REPO=%~dp0.."
cd /d "%REPO%"

set "PYTHON="
where python >nul 2>&1 && set "PYTHON=python"
if not defined PYTHON (
  where py >nul 2>&1 && set "PYTHON=py -3"
)
if not defined PYTHON (
  echo [错误] 未找到 Python。请安装 Python 3.10+ 并加入 PATH。
  pause
  exit /b 1
)

echo [hanimport] 检查 UnityPy …
%PYTHON% -c "import UnityPy" >nul 2>&1
if errorlevel 1 (
  echo [hanimport] UnityPy 未安装，正在安装依赖 …
  %PYTHON% -m pip install -r "%ROOT%scripts\requirements.txt"
  if errorlevel 1 (
    echo [错误] 依赖安装失败
    pause
    exit /b 1
  )
)

echo [hanimport] 编译 hanimport（若需要）…
cargo build -p hanimport >nul 2>&1

echo [hanimport] 启动网页界面 http://127.0.0.1:7821/
echo 关闭本窗口即可停止服务。
%PYTHON% "%ROOT%scripts\serve_web.py"
set "EXITCODE=%ERRORLEVEL%"
if not "%EXITCODE%"=="0" pause
exit /b %EXITCODE%
