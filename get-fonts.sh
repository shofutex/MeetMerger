#!/bin/bash
set -e

OUT_DIR=$1
FILENAME=Inter-4.1.zip

wget https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip
mkdir tmp
mv $FILENAME tmp/
cd tmp
unzip $FILENAME
cp extras/ttf/Inter-Regular.ttf "$OUT_DIR/"
cd ..
rm -rf tmp
