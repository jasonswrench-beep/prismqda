@echo off
setlocal
chcp 65001 >nul
title PrismQDA - test
cd /d "%~dp0"

echo.
echo ============================================================
echo   PrismQDA: run all the tests and the license check
echo ============================================================
echo.
echo This runs on the efficiency cores of this PC only (CPUs 16 to 31),
echo because the performance cores crash programs here. It is about
echo twice as slow, and that is expected. Nothing is shown on screen
echo that you need to act on; the last line says whether it passed.
echo.

start "" /b /wait /affinity FFFF0000 cargo test --workspace > test-log.txt 2>&1
if errorlevel 1 goto failed

echo PASSED. Every test passed, and every dependency has an accepted license.
echo The full log is in test-log.txt.
goto end

:failed
echo FAILED. Open test-log.txt in this folder to see which test failed.

:end
echo.
pause
