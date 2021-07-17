#!/usr/bin/env bash
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

set -ex

cd /build

ROOT=$(pwd)

export PATH=/toolchains/bin:$PATH

docker-extract-sccache.sh

tar -xf binutils-${BINUTILS_VERSION}.tar.xz
tar -xf gcc-${GCC_10_3_VERSION}.tar.xz
tar -xf gmp-${GMP_VERSION}.tar.xz
tar -xf isl-${ISL_VERSION}.tar.bz2
tar -xf mpc-${MPC_VERSION}.tar.gz
tar -xf mpfr-${MPFR_VERSION}.tar.xz

pushd gcc-${GCC_10_3_VERSION}
ln -sf ../gmp-${GMP_VERSION} gmp
ln -sf ../isl-${ISL_VERSION} isl
ln -sf ../mpc-${MPC_VERSION} mpc
ln -sf ../mpfr-${MPFR_VERSION} mpfr
popd

SCCACHE_ERROR_LOG=~/sccache.txt SCCACHE_LOG=info sccache --start-server

export CC="sccache /usr/bin/gcc"
export CXX="sccache /usr/bin/g++"

# Build binutils first.

mkdir binutils-objdir
pushd binutils-objdir

STAGE_CC_WRAPPER=sccache \
    ../binutils-${BINUTILS_VERSION}/configure \
    --build=x86_64-unknown-linux-gnu \
    --prefix=/ \
    --enable-gold \
    --enable-plugins \
    --disable-nls \
    --with-sysroot=/

make -j ${PARALLEL}
make install -j `nproc` DESTDIR=/out/binutils
popd

sccache -s
sccache -z >/dev/null

export PATH=/out/binutils/bin:$PATH

mkdir gcc-objdir

pushd gcc-objdir

# We don't use GCC for anything other than building llvm/clang. So
# we can skip the 3 stage bootstrap to save time.
../gcc-${GCC_10_3_VERSION}/configure \
    --build=x86_64-unknown-linux-gnu \
    --prefix=/ \
    --disable-bootstrap \
    --enable-languages=c,c++ \
    --disable-nls \
    --disable-gnu-unique-object \
    --enable-__cxa_atexit \
    --with-sysroot=/

time make -j ${PARALLEL}
time make -j `nproc` install DESTDIR=/out/gcc
popd

sccache --stop-server
