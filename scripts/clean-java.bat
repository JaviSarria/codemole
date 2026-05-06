@echo off
rem clean-java.bat - cleans the Java parser Maven build.
cd /D "%~dp0..\parsers\java"
call mvn -q clean
