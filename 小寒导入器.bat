@echo off
setlocal
chcp 65001 >nul
cd /d "%~dp0"
call "hanimport\启动小寒导入器.bat" %*
