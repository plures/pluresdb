@echo off
REM Rusty Gun Docker Runner Script for Windows
REM Makes it easy to run Rusty Gun with Docker

setlocal enabledelayedexpansion

REM Default values
set IMAGE=rusty-gun/rusty-gun:latest
set API_PORT=34567
set WEB_PORT=34568
set DATA_VOLUME=rusty-gun-data
set CONFIG_VOLUME=rusty-gun-config
set CONTAINER_NAME=rusty-gun
set COMMAND=start
set FOREGROUND=false
set NO_PULL=false

REM Parse command line arguments
:parse_args
if "%~1"=="" goto :execute_command
if "%~1"=="start" set COMMAND=start & shift & goto :parse_args
if "%~1"=="stop" set COMMAND=stop & shift & goto :parse_args
if "%~1"=="restart" set COMMAND=restart & shift & goto :parse_args
if "%~1"=="logs" set COMMAND=logs & shift & goto :parse_args
if "%~1"=="status" set COMMAND=status & shift & goto :parse_args
if "%~1"=="clean" set COMMAND=clean & shift & goto :parse_args
if "%~1"=="help" set COMMAND=help & shift & goto :parse_args
if "%~1"=="--api-port" set API_PORT=%~2 & shift & shift & goto :parse_args
if "%~1"=="--web-port" set WEB_PORT=%~2 & shift & shift & goto :parse_args
if "%~1"=="--image" set IMAGE=%~2 & shift & shift & goto :parse_args
if "%~1"=="--no-pull" set NO_PULL=true & shift & goto :parse_args
if "%~1"=="--detach" set FOREGROUND=false & shift & goto :parse_args
if "%~1"=="--foreground" set FOREGROUND=true & shift & goto :parse_args
if "%~1"=="--help" set COMMAND=help & shift & goto :parse_args
if "%~1"=="-h" set COMMAND=help & shift & goto :parse_args
echo [ERROR] Unknown option: %~1
goto :show_usage

:show_usage
echo Rusty Gun Docker Runner
echo.
echo Usage: %0 [COMMAND] [OPTIONS]
echo.
echo Commands:
echo   start     Start Rusty Gun (default)
echo   stop      Stop Rusty Gun
echo   restart   Restart Rusty Gun
echo   logs      Show logs
echo   status    Show status
echo   clean     Remove containers and volumes
echo   help      Show this help
echo.
echo Options:
echo   --api-port PORT     API port (default: 34567)
echo   --web-port PORT     Web UI port (default: 34568)
echo   --image IMAGE       Docker image (default: rusty-gun/rusty-gun:latest)
echo   --no-pull           Don't pull latest image
echo   --detach            Run in background (default)
echo   --foreground        Run in foreground
echo.
echo Examples:
echo   %0 start
echo   %0 start --api-port 8080 --web-port 8081
echo   %0 logs
echo   %0 stop
goto :eof

:check_docker
docker info >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Docker is not running. Please start Docker and try again.
    exit /b 1
)
goto :eof

:pull_image
if "%NO_PULL%"=="true" goto :eof
echo [INFO] Pulling latest image...
docker pull %IMAGE%
goto :eof

:start_rusty_gun
echo [INFO] Starting Rusty Gun...

REM Check if container already exists
docker ps -a --format "table {{.Names}}" | findstr /C:"%CONTAINER_NAME%" >nul
if not errorlevel 1 (
    echo [WARNING] Container '%CONTAINER_NAME%' already exists. Stopping and removing it first...
    docker stop %CONTAINER_NAME% >nul 2>&1
    docker rm %CONTAINER_NAME% >nul 2>&1
)

REM Create volumes if they don't exist
docker volume create %DATA_VOLUME% >nul 2>&1
docker volume create %CONFIG_VOLUME% >nul 2>&1

REM Start container
if "%FOREGROUND%"=="true" (
    docker run --name %CONTAINER_NAME% -p %API_PORT%:34567 -p %WEB_PORT%:34568 -v %DATA_VOLUME%:/app/data -v %CONFIG_VOLUME%:/app/config -e RUSTY_GUN_PORT=34567 -e RUSTY_GUN_WEB_PORT=34568 -e RUSTY_GUN_HOST=0.0.0.0 -e RUSTY_GUN_DATA_DIR=/app/data -e RUSTY_GUN_CONFIG_DIR=/app/config %IMAGE%
) else (
    docker run -d --name %CONTAINER_NAME% -p %API_PORT%:34567 -p %WEB_PORT%:34568 -v %DATA_VOLUME%:/app/data -v %CONFIG_VOLUME%:/app/config -e RUSTY_GUN_PORT=34567 -e RUSTY_GUN_WEB_PORT=34568 -e RUSTY_GUN_HOST=0.0.0.0 -e RUSTY_GUN_DATA_DIR=/app/data -e RUSTY_GUN_CONFIG_DIR=/app/config --restart unless-stopped %IMAGE%
    echo [SUCCESS] Rusty Gun started successfully!
    echo [INFO] API: http://localhost:%API_PORT%
    echo [INFO] Web UI: http://localhost:%WEB_PORT%
    echo [INFO] Container name: %CONTAINER_NAME%
    echo [INFO] To view logs: %0 logs
    echo [INFO] To stop: %0 stop
)
goto :eof

:stop_rusty_gun
echo [INFO] Stopping Rusty Gun...
docker ps --format "table {{.Names}}" | findstr /C:"%CONTAINER_NAME%" >nul
if not errorlevel 1 (
    docker stop %CONTAINER_NAME%
    echo [SUCCESS] Rusty Gun stopped successfully!
) else (
    echo [WARNING] Rusty Gun is not running.
)
goto :eof

:restart_rusty_gun
echo [INFO] Restarting Rusty Gun...
call :stop_rusty_gun
timeout /t 2 /nobreak >nul
call :start_rusty_gun
goto :eof

:show_logs
docker ps -a --format "table {{.Names}}" | findstr /C:"%CONTAINER_NAME%" >nul
if not errorlevel 1 (
    echo [INFO] Showing logs for %CONTAINER_NAME%...
    docker logs -f %CONTAINER_NAME%
) else (
    echo [ERROR] Container '%CONTAINER_NAME%' not found.
    exit /b 1
)
goto :eof

:show_status
echo [INFO] Rusty Gun Status:
echo.
docker ps --format "table {{.Names}}" | findstr /C:"%CONTAINER_NAME%" >nul
if not errorlevel 1 (
    echo [SUCCESS] Container is running
    echo.
    docker ps --filter "name=%CONTAINER_NAME%" --format "table {{.Names}}	{{.Status}}	{{.Ports}}"
    echo.
    echo [INFO] API: http://localhost:%API_PORT%
    echo [INFO] Web UI: http://localhost:%WEB_PORT%
) else (
    echo [WARNING] Container is not running
    docker ps -a --format "table {{.Names}}" | findstr /C:"%CONTAINER_NAME%" >nul
    if not errorlevel 1 (
        echo.
        docker ps -a --filter "name=%CONTAINER_NAME%" --format "table {{.Names}}	{{.Status}}	{{.Ports}}"
    )
)
goto :eof

:clean_up
echo [INFO] Cleaning up Rusty Gun containers and volumes...

REM Stop and remove container
docker ps -a --format "table {{.Names}}" | findstr /C:"%CONTAINER_NAME%" >nul
if not errorlevel 1 (
    docker stop %CONTAINER_NAME% >nul 2>&1
    docker rm %CONTAINER_NAME% >nul 2>&1
    echo [SUCCESS] Container removed
)

REM Remove volumes
docker volume ls --format "table {{.Name}}" | findstr /C:"%DATA_VOLUME%" >nul
if not errorlevel 1 (
    docker volume rm %DATA_VOLUME% >nul 2>&1
    echo [SUCCESS] Data volume removed
)

docker volume ls --format "table {{.Name}}" | findstr /C:"%CONFIG_VOLUME%" >nul
if not errorlevel 1 (
    docker volume rm %CONFIG_VOLUME% >nul 2>&1
    echo [SUCCESS] Config volume removed
)

echo [SUCCESS] Cleanup completed!
goto :eof

:execute_command
call :check_docker
if errorlevel 1 exit /b 1

if "%COMMAND%"=="start" (
    call :pull_image
    call :start_rusty_gun
) else if "%COMMAND%"=="stop" (
    call :stop_rusty_gun
) else if "%COMMAND%"=="restart" (
    call :pull_image
    call :restart_rusty_gun
) else if "%COMMAND%"=="logs" (
    call :show_logs
) else if "%COMMAND%"=="status" (
    call :show_status
) else if "%COMMAND%"=="clean" (
    call :clean_up
) else if "%COMMAND%"=="help" (
    call :show_usage
) else (
    echo [ERROR] Unknown command: %COMMAND%
    call :show_usage
    exit /b 1
)

endlocal
