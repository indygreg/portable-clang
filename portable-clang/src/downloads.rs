// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    anyhow::{Context, Result},
    once_cell::sync::Lazy,
    slog::Logger,
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    },
    tugger_common::http::{download_to_path, RemoteContent},
};

pub static DOWNLOADS: Lazy<BTreeMap<&str, RemoteContent>> = Lazy::new(|| {
    BTreeMap::from_iter([
        ("binutils", RemoteContent {
            name: "binutils".to_string(),
            url: "https://ftp.gnu.org/gnu/binutils/binutils-2.36.1.tar.xz".to_string(),
            sha256: "e81d9edf373f193af428a0f256674aea62a9d74dfe93f65192d4eae030b0f3b0".to_string(),
        }),
        ("clang", RemoteContent {
            name: "clang".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/clang-13.0.0.src.tar.xz".to_string(),
            sha256: "5d611cbb06cfb6626be46eb2f23d003b2b80f40182898daa54b1c4e8b5b9e17e".to_string(),
        }),
        ("clang-tools-extra", RemoteContent {
            name: "clang-tools-extra".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/clang-tools-extra-13.0.0.src.tar.xz".to_string(),
            sha256: "428b6060a28b22adf0cdf5d827abbc2ba81809f4661ede3d02b1d3fedaa3ead5".to_string(),
        }),
        ("cmake-linux_x86_64", RemoteContent {
            name: "cmake-linux_x86_64".to_string(),
            url: "https://github.com/Kitware/CMake/releases/download/v3.21.4/cmake-3.21.4-linux-x86_64.tar.gz".to_string(),
            sha256: "eddba9da5b60e0b5ec5cbb1a65e504d776e247573204df14f6d004da9bc611f9".to_string(),
        }),
        ("compiler-rt", RemoteContent {
            name: "compiler-rt".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/compiler-rt-13.0.0.src.tar.xz".to_string(),
            sha256: "4c3602d76c7868a96b30c36165c4b7643e2a20173fced7e071b4baeb2d74db3f".to_string()
        }),
        ("gcc-10_3", RemoteContent {
            name: "gcc-10_3".to_string(),
            url: "https://ftp.gnu.org/gnu/gcc/gcc-10.3.0/gcc-10.3.0.tar.xz".to_string(),
            sha256: "64f404c1a650f27fc33da242e1f2df54952e3963a49e06e73f6940f3223ac344".to_string(),
        }),
        ("gmp", RemoteContent {
            name: "gmp".to_string(),
            url: "https://ftp.gnu.org/gnu/gmp/gmp-6.1.2.tar.xz".to_string(),
            sha256: "87b565e89a9a684fe4ebeeddb8399dce2599f9c9049854ca8c0dfbdea0e21912".to_string(),
        }),
        ("isl", RemoteContent {
            name: "isl".to_string(),
            url: "https://gcc.gnu.org/pub/gcc/infrastructure/isl-0.18.tar.bz2".to_string(),
            sha256: "6b8b0fd7f81d0a957beb3679c81bbb34ccc7568d5682844d8924424a0dadcb1b".to_string(),
        }),
        ("libcxx", RemoteContent {
            name: "libcxx".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/libcxx-13.0.0.src.tar.xz".to_string(),
            sha256: "3682f16ce33bb0a8951fc2c730af2f9b01a13b71b2b0dc1ae1e7034c7d86ca1a".to_string()
        }),
        ("libcxxabi", RemoteContent {
            name: "libcxxabi".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/libcxxabi-13.0.0.src.tar.xz".to_string(),
            sha256: "becd5f1cd2c03cd6187558e9b4dc8a80b6d774ff2829fede88aa1576c5234ce3".to_string()
        }),
        ("libunwind", RemoteContent {
            name: "libunwind".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/libunwind-13.0.0.src.tar.xz".to_string(),
            sha256: "36f819091216177a61da639244eda67306ccdd904c757d70d135e273278b65e1".to_string()
        }),
        ("lld", RemoteContent {
            name: "lld".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/lld-13.0.0.src.tar.xz".to_string(),
            sha256: "20d1900bcd64ff62047291f6edb6ba2fed34d782675ff68713bf0c2fc9e69386".to_string(),
        }),
        ("llvm", RemoteContent {
            name: "llvm".to_string(),
            url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/llvm-13.0.0.src.tar.xz".to_string(),
            sha256: "408d11708643ea826f519ff79761fcdfc12d641a2510229eec459e72f8163020".to_string()
        }),
        ("mpc", RemoteContent {
            name: "mpc".to_string(),
            url: "http://www.multiprecision.org/downloads/mpc-1.0.3.tar.gz".to_string(),
            sha256: "617decc6ea09889fb08ede330917a00b16809b8db88c29c31bfbb49cbf88ecc3".to_string(),
        }),
        ("mpfr", RemoteContent {
            name: "mpfr".to_string(),
            url: "https://ftp.gnu.org/gnu/mpfr/mpfr-3.1.6.tar.xz".to_string(),
            sha256: "7a62ac1a04408614fccdc506e4844b10cf0ad2c2b1677097f8f35d3a1344a950".to_string(),
        }),
        ("ninja-linux_x86_64", RemoteContent {
            name: "ninja-linux_x86_64".to_string(),
            url: "https://github.com/ninja-build/ninja/releases/download/v1.10.2/ninja-linux.zip".to_string(),
            sha256: "763464859c7ef2ea3a0a10f4df40d2025d3bb9438fcb1228404640410c0ec22d".to_string(),
        }),
        ("python-linux_x86_64", RemoteContent {
            name: "python-linux_x86_64".to_string(),
            url: "https://github.com/indygreg/python-build-standalone/releases/download/20211017/cpython-3.9.7-x86_64-unknown-linux-gnu-install_only-20211017T1616.tar.gz".to_string(),
            sha256: "a92dfd11be92c8b5f7b50953bdb5864456d68d316f73d9cfb4bde33a68ac8239".to_string(),
        }),
        ("sccache-linux_x86_64", RemoteContent {
            name: "sccache-linux_x86_64".to_string(),
            url: "https://github.com/mozilla/sccache/releases/download/v0.2.15/sccache-v0.2.15-x86_64-unknown-linux-musl.tar.gz".to_string(),
            sha256: "e5d03a9aa3b9fac7e490391bbe22d4f42c840d31ef9eaf127a03101930cbb7ca".to_string(),
        }),
   ])
});

/// [RemoteContent] records for GCC source artifacts.
pub fn gcc_source_remote_contents() -> Vec<&'static RemoteContent> {
    DOWNLOADS
        .iter()
        .filter_map(|(name, record)| {
            if matches!(
                *name,
                "binutils" | "gcc-10_3" | "gmp" | "isl" | "mpc" | "mpfr"
            ) {
                Some(record)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

/// [RemoteContent] records for LLVM source artifacts.
pub fn llvm_source_remote_contents() -> Vec<&'static RemoteContent> {
    DOWNLOADS
        .iter()
        .filter_map(|(name, record)| {
            if matches!(
                *name,
                "clang"
                    | "clang-tools-extra"
                    | "compiler-rt"
                    | "libcxx"
                    | "libcxxabi"
                    | "libunwind"
                    | "lld"
                    | "llvm"
            ) {
                Some(record)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

/// [RemoteContent] records for support tools.
pub fn support_linux_x86_64_remote_contents() -> Vec<&'static RemoteContent> {
    DOWNLOADS
        .iter()
        .filter_map(|(name, record)| {
            if matches!(
                *name,
                "cmake-linux_x86_64"
                    | "ninja-linux_x86_64"
                    | "python-linux_x86_64"
                    | "sccache-linux_x86_64"
            ) {
                Some(record)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

/// Fetch multiple [RemoteContent] records to a destination directory.
pub fn fetch_records(
    logger: &Logger,
    records: &[&RemoteContent],
    dest_path: &Path,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dest_path).context("creating destination directory")?;
    let mut res = vec![];

    for record in records {
        let filename = record.url.rsplit_once('/').expect("URL should have /").1;

        let p = dest_path.join(filename);

        download_to_path(logger, record, &p).context("downloading remote content")?;

        let lock_path = p.with_extension("lock");
        if lock_path.exists() {
            std::fs::remove_file(&lock_path).context("removing lock file")?;
        }

        res.push(p);
    }

    Ok(res)
}

/// Fetch GCC source tarballs to the specified destination path.
pub fn fetch_gcc_sources(logger: &Logger, dest_path: &Path) -> Result<Vec<PathBuf>> {
    fetch_records(logger, &gcc_source_remote_contents(), dest_path)
}

/// Fetch LLVM source tarballs to the specified destination path.
pub fn fetch_llvm_sources(logger: &Logger, dest_path: &Path) -> Result<Vec<PathBuf>> {
    fetch_records(logger, &llvm_source_remote_contents(), dest_path)
}

/// Fetch artifacts needed as support files for Linux x86_64 builds.
pub fn fetch_linux_x86_64_support(logger: &Logger, dest_path: &Path) -> Result<Vec<PathBuf>> {
    fetch_records(logger, &support_linux_x86_64_remote_contents(), dest_path)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn gcc_source_downloads() -> Result<()> {
        let logger = crate::logging::logger();
        let td = tempfile::TempDir::new()?;

        fetch_gcc_sources(&logger, td.path())?;

        Ok(())
    }

    #[test]
    fn llvm_source_download() -> Result<()> {
        let logger = crate::logging::logger();
        let td = tempfile::TempDir::new()?;

        fetch_llvm_sources(&logger, td.path())?;

        Ok(())
    }
}
