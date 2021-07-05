#!/usr/bin/env bash

# Script executed in Docker to build a single configuration of glibc.

set -ex

COMPILER=$1
GLIBC=$2

/usr/bin/docker-extract-sccache.sh
export PATH=/toolchains/bin:$PATH

SCCACHE_ERROR_LOG=~/sccache.txt SCCACHE_LOG=info sccache --start-server

export CC="sccache gcc"
export CXX="sccache g++"

time build-many-glibcs.py -j ${PARALLEL} /build compilers "${COMPILER}"
sccache -s
sccache -z >/dev/null

time build-many-glibcs.py -j ${PARALLEL} /build glibcs "${GLIBC}"
sccache --stop-server

cp -a install/glibcs/"${GLIBC}" /out/
