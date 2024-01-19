# KViz

A set of music visualizer experiments in rust.

## Running in dev

To run in the dev environment, run:

```bash
./run-dev.sh
```

Be sure you have built ffmpeg beforehand though, using:

```bash
./build-ffmpeg.sh
```

## Building a release

This currently only builds on Linux. I do not currently plan to support other operating systems.

To build, run:

```bash
./build-ffmpeg.sh
./build-release.sh
./build-dist.sh
```

Theoretically, building on Max OS X would involve using `install_name_tool` to change the `rpath` of the release
executable to be local like what is done on Linux currently. On Windows, one would just need to put all the `.dll` files
in the same directory as the executable and that should get it to work.
