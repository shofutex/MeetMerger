@echo off
set FILENAME=Inter-4.1.zip

curl -L https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip -o %FILENAME%
mkdir tmp
move %FILENAME% tmp\
cd tmp
tar -xf %FILENAME%
mkdir ..\fonts
copy extras\ttf\Inter-Regular.ttf ..\fonts\
cd ..
rmdir /s /q tmp
