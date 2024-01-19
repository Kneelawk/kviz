#!/usr/bin/env bash

if [ ! -f "target/release/kviz" ]; then
  echo "Please run build-release.sh first."
  exit -1
fi

mkdir -p dist/lib
cp "target/release/kviz" "dist/"
patchelf --force-rpath --set-rpath '$ORIGIN/lib' dist/kviz
cp -r ffmpeg/lib/*.so.* "dist/lib/"
