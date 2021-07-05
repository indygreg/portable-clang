#!/usr/bin/env bash

# Script to initialize a Docker image to build glibc.

set -ex

pushd /build

patch -p1 < build-many-glibcs-sccache.patch

cp build-many-glibcs.py /usr/bin/

popd

su - build -c "build-many-glibcs.py --shallow /build checkout glibc-vcs-2.34"
su - build -c "build-many-glibcs.py /build host-libraries"
