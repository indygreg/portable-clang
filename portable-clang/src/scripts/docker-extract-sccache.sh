#!/usr/bin/env bash

set -ex

VERSION_STRING=sccache-v${SCCACHE_LINUX_X86_64_VERSION}-x86_64-unknown-linux-musl

mkdir -p /toolchains/bin

tar -xf /build/${VERSION_STRING}.tar.gz
mv ${VERSION_STRING}/sccache /toolchains/bin/
chmod +x /toolchains/bin/sccache
