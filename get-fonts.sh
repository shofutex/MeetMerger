#!/bin/bash

FILENAME=Inter-4.1.zip

wget https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip
mkdir tmp
mv $FILENAME tmp/
cd tmp
unzip $FILENAME
mkdir -p ../fonts
cp extras/ttf/Inter-Regular.ttf ../fonts/
cd ..
rm -rf tmp
