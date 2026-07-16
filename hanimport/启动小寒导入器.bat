@echo off
setlocal EnableExtensions
chcp 65001 >nul

set "ROOT=%~dp0"
set "REPO=%~dp0.."
cd /d "%REPO%"

:menu
cls
echo.
echo  ╔══════════════════════════════════════╗
echo  ║         小寒导入器 hanimport         ║
echo  ║   碧蓝航线 AssetBundle 模型解包工具   ║
echo  ╚══════════════════════════════════════╝
echo.
echo   1. 打开网页界面（推荐）
echo   2. 命令行解包 — data/model 目录
echo   3. 命令行解包 — 自定义路径
echo   4. 生成模型 JSON 配置（data/live2d）
echo   5. 安装 / 检查依赖
echo   6. 打包便携版 exe
echo   0. 退出
echo.
set /p CHOICE=请选择 [0-6]: 

if "%CHOICE%"=="1" goto web
if "%CHOICE%"=="2" goto unpack_model
if "%CHOICE%"=="3" goto unpack_custom
if "%CHOICE%"=="4" goto config_live2d
if "%CHOICE%"=="5" goto setup
if "%CHOICE%"=="6" goto release
if "%CHOICE%"=="0" exit /b 0
goto menu

:web
call "%ROOT%启动网页版.bat"
goto menu

:unpack_model
set "INPUT=%REPO%\data\model\azurlane\custom"
set "OUTPUT=%REPO%\data\model\unpacked"
if not exist "%INPUT%" (
  echo [错误] 默认输入目录不存在: %INPUT%
  pause
  goto menu
)
call :run_unpack "%INPUT%" "%OUTPUT%"
goto menu

:unpack_custom
set /p INPUT=输入路径（文件或目录）: 
if "%INPUT%"=="" goto menu
set /p OUTPUT=输出目录（留空自动）: 
if "%OUTPUT%"=="" (
  call :run_unpack "%INPUT%"
) else (
  call :run_unpack "%INPUT%" "%OUTPUT%"
)
goto menu

:config_live2d
set "INPUT=%REPO%\data\live2d"
if not exist "%INPUT%" (
  echo [错误] 目录不存在: %INPUT%
  pause
  goto menu
)
set "EXE=%REPO%\hanpet\src-tauri\target\release\hanimport.exe"
if exist "%EXE%" (
  "%EXE%" config --input "%INPUT%"
) else (
  cargo run -p hanimport -- config --input "%INPUT%"
)
echo.
pause
goto menu

:setup
call "%ROOT%scripts\setup-env.bat"
goto menu

:release
powershell -NoProfile -ExecutionPolicy Bypass -File "%REPO%\scripts\build-hanimport-release.ps1"
pause
goto menu

:run_unpack
set "IN=%~1"
set "OUT=%~2"
set "EXE=%REPO%\hanpet\src-tauri\target\release\hanimport.exe"
if exist "%EXE%" (
  if "%OUT%"=="" (
    "%EXE%" unpack --input "%IN%"
  ) else (
    "%EXE%" unpack --input "%IN%" --output "%OUT%"
  )
) else (
  if "%OUT%"=="" (
    cargo run -p hanimport -- unpack --input "%IN%"
  ) else (
    cargo run -p hanimport -- unpack --input "%IN%" --output "%OUT%"
  )
)
echo.
pause
goto :eof
