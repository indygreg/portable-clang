#!/usr/bin/env bash

set -ex

ROOT=$(pwd)

docker-extract-sccache.sh

tar -C /toolchains -xf /inputs/binutils.tar
tar -C /toolchains -xf /inputs/gcc.tar

mkdir /toolchains/cmake
tar -C /toolchains/cmake --strip-components=1 -xf ${ROOT}/cmake-${CMAKE_LINUX_X86_64_VERSION}-linux-x86_64.tar.gz

mkdir /toolchains/ninja
unzip ${ROOT}/ninja-linux.zip
mv ninja /toolchains/bin/

tar -C /toolchains -xf ${ROOT}/${PYTHON_LINUX_X86_64_VERSION}.tar.gz

export PATH=/toolchains/cmake/bin:/toolchains/bin:/toolchains/python/bin:/toolchains/binutils/bin:/toolchains/gcc/bin:$PATH

mkdir llvm
pushd llvm
tar --strip-components=1 -xf ${ROOT}/llvm-${CLANG_VERSION}.src.tar.xz
popd

mkdir llvm/tools/clang
pushd llvm/tools/clang
tar --strip-components=1 -xf ${ROOT}/clang-${CLANG_VERSION}.src.tar.xz
popd

mkdir llvm/tools/lld
pushd llvm/tools/lld
tar --strip-components=1 -xf ${ROOT}/lld-${CLANG_VERSION}.src.tar.xz
popd

mkdir llvm/projects/compiler-rt
pushd llvm/projects/compiler-rt
tar --strip-components=1 -xf ${ROOT}/compiler-rt-${CLANG_VERSION}.src.tar.xz
popd

mkdir llvm/projects/libcxx
pushd llvm/projects/libcxx
tar --strip-components=1 -xf ${ROOT}/libcxx-${CLANG_VERSION}.src.tar.xz
popd

mkdir llvm/projects/libcxxabi
pushd llvm/projects/libcxxabi
tar --strip-components=1 -xf ${ROOT}/libcxxabi-${CLANG_VERSION}.src.tar.xz
popd

mkdir libunwind
pushd libunwind
tar --strip-components=1 -xf ${ROOT}/libunwind-${CLANG_VERSION}.src.tar.xz
popd

mkdir llvm-objdir
pushd llvm-objdir

SCCACHE_ERROR_LOG=~/sccache.txt SCCACHE_LOG=info sccache --start-server
EXTRA_FLAGS="-DCMAKE_C_COMPILER_LAUNCHER=sccache -DCMAKE_CXX_COMPILER_LAUNCHER=sccache"

# Stage 1: Build with GCC.
mkdir stage1
pushd stage1
cmake \
    -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/toolchains/clang-stage1 \
    -DCMAKE_C_COMPILER=gcc \
    -DCMAKE_CXX_COMPILER=g++ \
    -DCMAKE_ASM_COMPILER=gcc \
    -DCMAKE_CXX_FLAGS="-Wno-cast-function-type" \
    -DCMAKE_EXE_LINKER_FLAGS="-Wl,-Bsymbolic-functions" \
    -DCMAKE_SHARED_LINKER_FLAGS="-Wl,-Bsymbolic-functions" \
    -DLLVM_TARGETS_TO_BUILD=X86 \
    -DLLVM_TOOL_LIBCXX_BUILD=ON \
    -DLIBCXX_LIBCPPABI_VERSION="" \
    -DLLVM_BINUTILS_INCDIR=/toolchains/binutils/include \
    -DLLVM_LINK_LLVM_DYLIB=ON \
    -DLLVM_INSTALL_UTILS=ON \
    ${EXTRA_FLAGS} \
    ../../llvm

LD_LIBRARY_PATH=/toolchains/gcc/lib64 ninja -j ${PARALLEL} install

sccache -s
sccache -z >/dev/null

mkdir -p /toolchains/clang-stage1/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}
cp -a /toolchains/gcc/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}/* /toolchains/clang-stage1/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}/
cp -a /toolchains/gcc/lib64/* /toolchains/clang-stage1/lib/
mkdir -p /toolchains/clang-stage1/lib32
cp -a /toolchains/gcc/lib32/* /toolchains/clang-stage1/lib32/
cp -a /toolchains/gcc/include/* /toolchains/binutils/include/* /toolchains/clang-stage1/include/

popd

# Stage 2: Build with GCC built Clang.
mkdir stage2
pushd stage2
cmake \
    -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/toolchains/clang-stage2 \
    -DCMAKE_C_COMPILER=/toolchains/clang-stage1/bin/clang \
    -DCMAKE_CXX_COMPILER=/toolchains/clang-stage1/bin/clang++ \
    -DCMAKE_ASM_COMPILER=/toolchains/clang-stage1/bin/clang \
    -DCMAKE_C_FLAGS="-fPIC" \
    -DCMAKE_CXX_FLAGS="-fPIC -Qunused-arguments -L/toolchains/clang-stage1/lib" \
    -DCMAKE_EXE_LINKER_FLAGS="-Wl,-Bsymbolic-functions -L/toolchains/clang-stage1/lib" \
    -DCMAKE_SHARED_LINKER_FLAGS="-Wl,-Bsymbolic-functions -L/toolchains/clang-stage1/lib" \
    -DLLVM_TARGETS_TO_BUILD=X86 \
    -DLLVM_TOOL_LIBCXX_BUILD=ON \
    -DLIBCXX_LIBCPPABI_VERSION="" \
    -DLLVM_BINUTILS_INCDIR=/toolchains/binutils/include \
    -DLLVM_LINK_LLVM_DYLIB=ON \
    -DLLVM_INSTALL_UTILS=ON \
    ${EXTRA_FLAGS} \
    ../../llvm

LD_LIBRARY_PATH=/toolchains/clang-stage1/lib ninja -j ${PARALLEL} install

sccache -s
sccache -z >/dev/null

mkdir -p /toolchains/clang-stage2/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}
cp -a /toolchains/gcc/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}/* /toolchains/clang-stage2/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}/
cp -a /toolchains/gcc/lib64/* /toolchains/clang-stage2/lib/
mkdir -p /toolchains/clang-stage2/lib32
cp -a /toolchains/gcc/lib32/* /toolchains/clang-stage2/lib32/
cp -a /toolchains/gcc/include/* /toolchains/binutils/include/* /toolchains/clang-stage2/include/

popd

# Stage 3: Build with Clang built Clang.
#
# We remove LLVM_TARGETS_TO_BUILD from this configuration, enabling
# support for all targets. The stage 1 and 2 builds don't benefit from
# non-native target support, which is why we exclude host target support
# above.
#
# We also use -march to enable use of more modern ISAs.

OUT_DIR=/out/clang

mkdir stage3
pushd stage3
cmake \
    -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=${OUT_DIR} \
    -DCMAKE_C_COMPILER=/toolchains/clang-stage2/bin/clang \
    -DCMAKE_CXX_COMPILER=/toolchains/clang-stage2/bin/clang++ \
    -DCMAKE_ASM_COMPILER=/toolchains/clang-stage2/bin/clang \
    -DCMAKE_C_FLAGS="-fPIC -march=x86-64-v3" \
    -DCMAKE_CXX_FLAGS="-fPIC -march=x86-64-v3 -Qunused-arguments -L/toolchains/clang-stage2/lib" \
    -DCMAKE_EXE_LINKER_FLAGS="-Wl,-Bsymbolic-functions -L/toolchains/clang-stage2/lib" \
    -DCMAKE_SHARED_LINKER_FLAGS="-Wl,-Bsymbolic-functions -L/toolchains/clang-stage2/lib" \
    -DLLVM_TOOL_LIBCXX_BUILD=ON \
    -DLIBCXX_LIBCPPABI_VERSION="" \
    -DLLVM_BINUTILS_INCDIR=/toolchains/binutils/include \
    -DLLVM_LINK_LLVM_DYLIB=ON \
    -DLLVM_INSTALL_UTILS=ON \
    ${EXTRA_FLAGS} \
    ../../llvm

LD_LIBRARY_PATH=/toolchains/clang-stage2/lib DESTDIR=/out ninja -j ${PARALLEL} install

sccache --stop-server

mkdir -p ${OUT_DIR}/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}
cp -a /toolchains/gcc/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}/* ${OUT_DIR}/lib/gcc/x86_64-unknown-linux-gnu/${GCC_VERSION}/
cp -a /toolchains/gcc/lib64/* ${OUT_DIR}/lib/
mkdir -p ${OUT_DIR}/lib32/
cp -a /toolchains/gcc/lib32/* ${OUT_DIR}/lib32/
cp -a /toolchains/gcc/include/* /toolchains/binutils/include/* ${OUT_DIR}/include/

popd

# Move out of objdir
popd
