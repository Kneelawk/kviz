#!/usr/bin/env bash

if [ ! -d "ffmpeg" ]; then
  echo "Please run build-ffmpeg.sh first."
  exit -1
fi

LD_LIBRARY_PATH="$(pwd)/ffmpeg/lib" FFMPEG_DIR="$(pwd)/ffmpeg" cargo run -- "$@"
