@echo off
rem build-java.bat - builds the Java parser fat-jar via Maven.
rem Usage: build-java.bat <profile_dir>  (e.g. release or debug)
set PROFILE_DIR=%1
if "%PROFILE_DIR%"=="" set PROFILE_DIR=release
cd /D "%~dp0..\parsers\java"
call mvn -q package -DskipTests
if errorlevel 1 exit /b 1
set DEST=%~dp0..\target\%PROFILE_DIR%
if not exist "%DEST%" mkdir "%DEST%"
copy /Y target\java-parser.jar "%DEST%\java-parser.jar" >nul
