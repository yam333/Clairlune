@echo off
rem Builds Aseprite v1.3.18.6 from source for personal use (Aseprite EULA: no redistribution).
rem Expects the source zip unpacked in TOOLS\aseprite and Skia m124 (Skia-Windows-Release-x64)
rem in TOOLS\skia. TOOLS defaults to a "tools" folder next to the repository.
rem Uses the VS 2022 toolset (14.44) inside VS 2026, the Windows 10.0.26100 SDK, the
rem CMake/Ninja bundled with Visual Studio, and the prebuilt Skia m124 package.

set "VS=C:\Program Files\Microsoft Visual Studio\18\Community"
if not defined TOOLS set "TOOLS=%~dp0..\..\tools"
set "SKIA=%TOOLS%\skia"

call "%VS%\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 -vcvars_ver=14.44 -winsdk=10.0.26100.0
if errorlevel 1 exit /b 1

rem Keep a MinGW toolchain out of CMake's sight; use Visual Studio's CMake and Ninja.
set "PATH=%PATH:C:\gcc-14.2.0\bin;=%"
set "PATH=%VS%\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;%VS%\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja;%PATH%"

where cl
cmake --version
ninja --version

cd /d "%TOOLS%\aseprite"
if not exist build mkdir build
cd build

cmake -G Ninja ^
  -DCMAKE_BUILD_TYPE=RelWithDebInfo ^
  -DCMAKE_C_COMPILER=cl -DCMAKE_CXX_COMPILER=cl ^
  -DCMAKE_IGNORE_PATH=C:/gcc-14.2.0/bin ^
  -DLAF_BACKEND=skia ^
  -DSKIA_DIR="%SKIA%" ^
  -DSKIA_LIBRARY_DIR="%SKIA%\out\Release-x64" ^
  -DSKIA_LIBRARY="%SKIA%\out\Release-x64\skia.lib" ^
  ..
if errorlevel 1 exit /b 1

ninja aseprite
if errorlevel 1 exit /b 1

echo BUILD OK: %TOOLS%\aseprite\build\bin\aseprite.exe
