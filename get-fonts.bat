@echo off
set OUT_DIR=%1
set FILENAME=Inter-4.1.zip

curl -L https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip -o %FILENAME%
mkdir tmp
move %FILENAME% tmp\
cd tmp
tar -xf %FILENAME%
copy extras\ttf\Inter-Regular.ttf "%OUT_DIR%\"
cd ..
rmdir /s /q tmp
