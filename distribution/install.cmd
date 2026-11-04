@echo off
setlocal

set "INSTALLER_URL=https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.ps1"
if defined H0X_INSTALLER_URL set "INSTALLER_URL=%H0X_INSTALLER_URL%"
rem Plain HTTP is accepted only for a local test server without userinfo.
set "CURL_PROTOCOL="
if /i "%INSTALLER_URL:~0,8%"=="https://" set "CURL_PROTOCOL=--proto =https --tlsv1.2"
if "%INSTALLER_URL:@=%"=="%INSTALLER_URL%" (
    if /i "%INSTALLER_URL:~0,17%"=="http://127.0.0.1:" set "CURL_PROTOCOL=--proto =http,https"
    if /i "%INSTALLER_URL:~0,17%"=="http://127.0.0.1/" set "CURL_PROTOCOL=--proto =http,https"
    if /i "%INSTALLER_URL:~0,17%"=="http://localhost:" set "CURL_PROTOCOL=--proto =http,https"
    if /i "%INSTALLER_URL:~0,17%"=="http://localhost/" set "CURL_PROTOCOL=--proto =http,https"
    if /i "%INSTALLER_URL:~0,13%"=="http://[::1]:" set "CURL_PROTOCOL=--proto =http,https"
    if /i "%INSTALLER_URL:~0,13%"=="http://[::1]/" set "CURL_PROTOCOL=--proto =http,https"
)
if not defined CURL_PROTOCOL (
    echo HyperOx installer URL must use HTTPS ^(plain HTTP only for 127.0.0.1, localhost or [::1]^). 1>&2
    exit /b 1
)
set "INSTALLER_PATH=%TEMP%\h0x-install-%RANDOM%-%RANDOM%.ps1"

curl.exe --fail --silent --show-error --location --connect-timeout 30 --speed-limit 1024 --speed-time 30 %CURL_PROTOCOL% --proto-redir =https --output "%INSTALLER_PATH%" -- "%INSTALLER_URL%"
if errorlevel 1 (
    del /f /q "%INSTALLER_PATH%" >nul 2>&1
    exit /b 1
)

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%INSTALLER_PATH%" %*
set "INSTALLER_EXIT=%ERRORLEVEL%"
del /f /q "%INSTALLER_PATH%" >nul 2>&1
exit /b %INSTALLER_EXIT%
