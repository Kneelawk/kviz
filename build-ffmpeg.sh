#!/usr/bin/env bash

rm -rf ffmpeg

set -ex

mkdir ffmpeg

pushd ffmpeg

FFMPEG_DIR="$(pwd)"

mkdir bin
mkdir include
mkdir lib
mkdir share
mkdir src

pushd src

# Install libopus
echo "### Installing LibOpus ###"
curl -L 'https://downloads.xiph.org/releases/opus/opus-1.4.tar.gz' -o opus.tar.gz
mkdir opus
tar -xf opus.tar.gz -C opus/ --strip-components=1
pushd opus
./configure --prefix="$FFMPEG_DIR" --disable-shared --disable-doc --disable-extra-programs --with-pic
make -j8
make install
popd


# Install libvpx
echo "### Installing LibVPX ###"
curl -L 'https://chromium.googlesource.com/webm/libvpx/+archive/v1.14.0.tar.gz' -o libvpx.tar.gz
mkdir libvpx
tar -xf libvpx.tar.gz -C libvpx/
pushd libvpx
./configure --prefix="$FFMPEG_DIR" --disable-shared --enable-pic --disable-examples --disable-tools --disable-docs --disable-unit-tests --enable-pic
make -j8
make install
popd

curl -L 'https://ffmpeg.org/releases/ffmpeg-6.1.1.tar.xz' -o ffmpeg.tar.xz
mkdir ffmpeg
tar -xf ffmpeg.tar.xz -C ffmpeg/ --strip-components=1
pushd ffmpeg

# annoyingly, rustc can't figure out how to link with ffmpeg if it references symbols from other static libraries,
# so we need to bundle ffmpeg and its deps into a shared library we can lug around with us
PATH="$FFMPEG_DIR/bin:$PATH" PKG_CONFIG_PATH="$FFMPEG_DIR/lib/pkgconfig" ./configure \
  --prefix="$FFMPEG_DIR" \
  --libdir="$FFMPEG_DIR/lib" \
  --incdir="$FFMPEG_DIR/include" \
  --bindir="$FFMPEG_DIR/bin" \
  --pkg-config-flags="--static" \
  --enable-pic \
  --enable-shared \
  --disable-static \
  --extra-cflags="-I$FFMPEG_DIR/include" \
  --extra-ldflags="-L$FFMPEG_DIR/lib -fPIC" \
  --extra-cxxflags="-L$FFMPEG_DIR/lib -I$FFMPEG_DIR/include" \
  --extra-libs="-lpthread -lm" \
  --ld="g++" \
  --bindir="$FFMPEG_DIR/bin" \
  --disable-autodetect \
  --enable-libopus \
  --enable-libvpx
make -j8
make install

popd

popd
