@echo off
rem Starts datalink-mp and the game with every log switched on, for an
rem internet-speed test. See docs/players/collecting-logs.md.
rem
rem Double-click this file from the Game folder (the one holding datalink-mp.exe
rem and dplayx.dll), or run:
rem   datalink-mp-collect-logs.bat [game.exe] [--early-acks] [--fast-wake] [--congestion NAME]
rem
rem Test switches, all off unless given (DATALINK_EARLY_ACKS, DATALINK_FAST_WAKE
rem and DATALINK_CONGESTION already set in the console count too):
rem   --early-acks       Helper acks reliable messages at once
rem   --fast-wake        the DLL polls every 1 ms
rem   --congestion NAME  bbr, cubic or newreno
setlocal
cd /d "%~dp0"

if not exist datalink-mp.exe (
  echo datalink-mp.exe is not in %CD%. Extract the whole archive into your Game folder.
  goto :fail
)
if not exist dplayx.dll (
  echo dplayx.dll is not in %CD%. Extract the whole archive into your Game folder.
  goto :fail
)

set "GAME="
set "NEXT_IS_CONGESTION="
for %%A in (%*) do call :arg "%%~A"
if defined NEXT_IS_CONGESTION (
  echo --congestion needs a name: bbr, cubic or newreno
  goto :fail
)
if not defined GAME (
  for %%G in (wtp.exe thinker.exe terran_PRACX.exe terranx.exe) do (
    if not defined GAME if exist "%%G" set "GAME=%%G"
  )
)
if not defined GAME (
  echo No game executable found here. Looked for wtp.exe, thinker.exe, terran_PRACX.exe, terranx.exe.
  echo Name it yourself:  %~nx0 game.exe
  goto :fail
)

set "LOG_DIR=%USERPROFILE%\dl-logs"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"

rem 1. The Helper, in its own window, with logging on. A window started with
rem    "start" inherits these variables, and so does the game below.
set "SMAC_HELPER_LOG_FILE=%LOG_DIR%\helper.log"
set "RUST_LOG=debug"
set "DATALINK_CAPTURE=%LOG_DIR%\capture"
set "DPLAYX_LOG_FILE=%LOG_DIR%\dll.log"
set "DPLAYX_RXTRACE=1"
echo DATALINK_EARLY_ACKS=%DATALINK_EARLY_ACKS% DATALINK_FAST_WAKE=%DATALINK_FAST_WAKE% DATALINK_CONGESTION=%DATALINK_CONGESTION%> "%LOG_DIR%\switches.txt"
echo Switches: & type "%LOG_DIR%\switches.txt"

start "datalink-mp" datalink-mp.exe
timeout /t 2 /nobreak >nul

rem 2. The game, with DLL logging on. Some launchers (wtp.exe, thinker.exe)
rem    start the real game and exit, so "start /wait" returns at once. Instead,
rem    wait a few seconds, then poll until none of the game's processes is left.
echo Starting %GAME% ...
start "" "%GAME%"
timeout /t 10 /nobreak >nul
:waitgame
set "RUNNING="
for %%G in ("%GAME%" wtp.exe thinker.exe terran_PRACX.exe terranx.exe) do (
  tasklist /nh /fi "imagename eq %%~G" 2>nul | find /i "%%~G" >nul && set "RUNNING=1"
)
if defined RUNNING (
  timeout /t 3 /nobreak >nul
  goto :waitgame
)

echo.
echo The game has closed. Quit datalink-mp with the Quit button on its page
echo (or close its console window) so its log is saved.
echo Logs are in %LOG_DIR%
explorer "%LOG_DIR%"
pause
exit /b 0

:fail
pause
exit /b 1

:arg
if defined NEXT_IS_CONGESTION (
  set "DATALINK_CONGESTION=%~1"
  set "NEXT_IS_CONGESTION="
  exit /b 0
)
if /i "%~1"=="--early-acks" ( set "DATALINK_EARLY_ACKS=1" & exit /b 0 )
if /i "%~1"=="--fast-wake" ( set "DATALINK_FAST_WAKE=1" & exit /b 0 )
if /i "%~1"=="--congestion" ( set "NEXT_IS_CONGESTION=1" & exit /b 0 )
if not defined GAME set "GAME=%~1"
exit /b 0
