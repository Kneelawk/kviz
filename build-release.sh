#!/usr/bin/env bash

if [ ! -d "ffmpeg" ]; then
  echo "Please run build-ffmpeg.sh first."
  exit -1
fi

FFMPEG_DIR="$(pwd)/ffmpeg" cargo build --release
