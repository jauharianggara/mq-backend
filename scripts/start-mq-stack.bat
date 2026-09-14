@echo off
REM ============================================================
REM MQ stack start (idempotent) - pola SGS start-sgs-stack
REM Komponen: BE mq-backend :8290 + worker + cloudflared cek
REM Service auto-start: MySQL (Laragon) + MQ-SeaweedFS (NSSM)
REM CATATAN: sengaja pakai findstr & ping-delay (bukan find/timeout)
REM          agar aman dari GNU-utils di PATH sesi terminal.
REM ============================================================
setlocal
set MQDIR=C:\Users\jauha\ZCodeProject\mq-backend
set MQEXE=%MQDIR%\target\x86_64-pc-windows-msvc\release\mq-backend.exe
set MQWORKER=%MQDIR%\target\x86_64-pc-windows-msvc\release\worker.exe

REM --- MySQL (port 3306) ---
netstat -ano | findstr ":3306 " | findstr "LISTENING" >nul 2>&1
if errorlevel 1 (
    echo [MQ] MySQL 3306 tidak jalan - CEK MANUAL (Laragon/service)
) else (
    echo [MQ] MySQL OK
)

REM --- SeaweedFS (service MQ-SeaweedFS, port 9000) ---
netstat -ano | findstr ":9000 " | findstr "LISTENING" >nul 2>&1
if errorlevel 1 (
    echo [MQ] SeaweedFS 9000 tidak jalan - restart service...
    C:\mq\nssm\nssm.exe restart MQ-SeaweedFS >nul 2>&1
    ping -n 6 127.0.0.1 >nul
) else (
    echo [MQ] SeaweedFS OK
)

REM --- BE mq-backend (proses) ---
tasklist 2>nul | findstr /i "mq-backend.exe" >nul
if errorlevel 1 (
    echo [MQ] Starting mq-backend...
    powershell -NoProfile -Command "Start-Process -FilePath '%MQEXE%' -WorkingDirectory '%MQDIR%' -WindowStyle Hidden -RedirectStandardOutput '%MQDIR%\run-mq.log' -RedirectStandardError '%MQDIR%\run-mq.err.log'"
    ping -n 4 127.0.0.1 >nul
) else (
    echo [MQ] mq-backend sudah jalan
)

REM --- worker (proses) ---
tasklist 2>nul | findstr /i "worker.exe" >nul
if errorlevel 1 (
    echo [MQ] Starting worker...
    powershell -NoProfile -Command "Start-Process -FilePath '%MQWORKER%' -WorkingDirectory '%MQDIR%' -WindowStyle Hidden -RedirectStandardOutput '%MQDIR%\run-worker.log' -RedirectStandardError '%MQDIR%\run-worker.err.log'"
) else (
    echo [MQ] worker sudah jalan
)

REM --- cloudflared (tunnel) ---
tasklist 2>nul | findstr /i "cloudflared.exe" >nul
if errorlevel 1 (
    echo [MQ] Starting cloudflared tunnel...
    powershell -NoProfile -Command "Start-Process -FilePath 'C:\Program Files (x86)\cloudflared\cloudflared.exe' -ArgumentList 'tunnel','run','pn-jagodigital' -WindowStyle Hidden"
    ping -n 9 127.0.0.1 >nul
) else (
    echo [MQ] cloudflared sudah jalan
)

REM --- smoke ---
curl -s -m 8 -o nul -w "[MQ] healthz lokal: %{http_code}\n" http://127.0.0.1:8290/healthz
curl -s -m 15 -o nul -w "[MQ] healthz publik: %{http_code}\n" https://mq-api.jagodigital.online/healthz
echo [MQ] selesai.
