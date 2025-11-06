@echo off
REM PluresDB Windows Launcher
REM This script starts the PluresDB server on Windows

echo.
echo =====================================
echo    PluresDB - Personal Database
echo =====================================
echo.
echo Starting PluresDB server...
echo.
echo Web UI will be available at:
echo   http://localhost:34568
echo.
echo API will be available at:
echo   http://localhost:34567
echo.
echo Press Ctrl+C to stop the server
echo.
echo =====================================
echo.

REM Start PluresDB server
pluresdb.exe serve --port 34567 --web-port 34568

REM If the server exits, pause to show error message
if errorlevel 1 (
    echo.
    echo =====================================
    echo ERROR: PluresDB failed to start
    echo =====================================
    echo.
    echo Common issues:
    echo - Port 34567 or 34568 is already in use
    echo - Missing web UI files
    echo - Insufficient permissions
    echo.
    echo Try running with different ports:
    echo   pluresdb.exe serve --port 8080 --web-port 8081
    echo.
    pause
)
