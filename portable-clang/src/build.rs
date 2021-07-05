// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/*! High level build logic. */

use crate::tar::tar_from_directory;
use {
    crate::docker::ZSTD_COMPRESSION_LEVEL,
    anyhow::{anyhow, Context, Result},
    slog::{warn, Logger},
    std::{
        io::Cursor,
        path::{Path, PathBuf},
    },
};

pub const GLIBC_GIT_URL: &str = "git://sourceware.org/git/glibc.git";

pub struct Environment {
    logger: Logger,
    cache_dir: PathBuf,
}

impl Environment {
    pub fn new(logger: Logger) -> Result<Self> {
        let cache_dir = if let Ok(p) = std::env::var("PCLANG_CACHE_DIR") {
            PathBuf::from(p)
        } else if let Some(cache_dir) = dirs::cache_dir() {
            cache_dir.join("pclang")
        } else {
            dirs::home_dir()
                .ok_or_else(|| {
                    anyhow!("could not resolve home dir as part of resolving cache directory")
                })?
                .join(".pclang")
                .join("cache")
        };

        Ok(Self { logger, cache_dir })
    }

    pub fn logger(&self) -> &Logger {
        &self.logger
    }

    fn docker_client(&self) -> Result<bollard::Docker> {
        crate::docker::docker_client()
    }

    pub async fn build_clang(
        &self,
        dest_dir: impl AsRef<Path>,
        image_path: Option<impl AsRef<Path>>,
        bootstrap_dir: Option<impl AsRef<Path>>,
    ) -> Result<()> {
        let dest_dir = dest_dir.as_ref();
        let bootstrap_dir = bootstrap_dir.map(|x| x.as_ref().to_path_buf());

        std::fs::create_dir_all(dest_dir)?;

        let (binutils_tar, gcc_tar) = if let Some(bootstrap_dir) = bootstrap_dir {
            let binutils_path = bootstrap_dir.join("binutils.tar.zst");
            let gcc_path = bootstrap_dir.join("gcc.tar.zst");

            warn!(
                &self.logger,
                "reading binutils from {}",
                binutils_path.display()
            );
            let binutils = std::fs::read(&binutils_path)?;
            warn!(&self.logger, "reading gcc from {}", gcc_path.display());
            let gcc = std::fs::read(&gcc_path)?;

            (binutils, gcc)
        } else {
            self.build_gcc(dest_dir, None).await?
        };

        let docker = self.docker_client()?;

        let image_id = if let Some(image_path) = image_path {
            let image_path = image_path.as_ref();
            let fh = std::fs::File::open(image_path).context("opening image archive")?;

            crate::docker::load_image_tar_zst(&self.logger, &docker, fh)
                .await
                .context("loading Docker image")?
        } else {
            crate::docker::build_image_clang(&self.logger, &docker, &self.cache_dir).await?
        };

        let clang_tar_zst = crate::docker::bootstrap_clang(
            &self.logger,
            &docker,
            &image_id,
            &binutils_tar,
            &gcc_tar,
            &self.cache_dir,
        )
        .await?;

        let clang_path = dest_dir.join("clang.tar.zst");
        std::fs::write(&clang_path, &clang_tar_zst)?;

        Ok(())
    }

    pub async fn build_gcc(
        &self,
        dest_dir: impl AsRef<Path>,
        image_path: Option<&Path>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let dest_dir = dest_dir.as_ref();

        std::fs::create_dir_all(dest_dir)?;

        let image_id = if let Some(image_path) = image_path {
            let fh = std::fs::File::open(image_path).context("opening image archive")?;

            crate::docker::load_image_tar_zst(&self.logger, &self.docker_client()?, fh)
                .await
                .context("loading Docker image")?
        } else {
            crate::docker::build_image_gcc(&self.logger, &self.docker_client()?, &self.cache_dir)
                .await?
        };

        let (binutils, gcc) = crate::docker::bootstrap_gcc(
            &self.logger,
            &self.docker_client()?,
            &image_id,
            &self.cache_dir,
        )
        .await?;

        let binutils_path = dest_dir.join("binutils.tar.zst");
        let gcc_path = dest_dir.join("gcc.tar.zst");

        std::fs::write(&binutils_path, &binutils)?;
        std::fs::write(&gcc_path, &gcc)?;

        Ok((binutils, gcc))
    }

    pub async fn docker_image_clang(&self, dest_dir: Option<impl AsRef<Path>>) -> Result<()> {
        let image_id =
            crate::docker::build_image_clang(&self.logger, &self.docker_client()?, &self.cache_dir)
                .await?;

        if let Some(dest_path) = dest_dir {
            let dest_path = dest_path.as_ref();
            let (in_size, out_size) = crate::docker::export_image_to_tar_zst(
                &self.docker_client()?,
                &image_id,
                dest_path,
            )
            .await
            .context("exporting Docker image to file")?;
            warn!(
                &self.logger,
                "wrote {}; compressed {} -> {} bytes",
                dest_path.display(),
                in_size,
                out_size
            );
        }

        Ok(())
    }

    pub async fn docker_image_gcc(&self, dest_dir: Option<impl AsRef<Path>>) -> Result<()> {
        let docker = self.docker_client()?;

        let image_id =
            crate::docker::build_image_gcc(&self.logger, &docker, &self.cache_dir).await?;

        if let Some(dest_path) = dest_dir {
            let dest_path = dest_path.as_ref();
            let (in_size, out_size) =
                crate::docker::export_image_to_tar_zst(&docker, &image_id, dest_path)
                    .await
                    .context("exporting Docker image to file")?;
            warn!(
                &self.logger,
                "wrote {}; compressed {} -> {} bytes",
                dest_path.display(),
                in_size,
                out_size
            );
        }

        Ok(())
    }

    pub fn fetch_glibc_git(&self) -> Result<()> {
        let repo_path = self.cache_dir.join("glibc.git");

        let repo = if repo_path.exists() {
            warn!(
                &self.logger,
                "opening Git repository at {}",
                repo_path.display()
            );
            git2::Repository::open(&repo_path).context("opening git repository")?
        } else {
            warn!(
                &self.logger,
                "creating bare Git repository at {}",
                repo_path.display()
            );
            std::fs::create_dir_all(&repo_path).context("creating directory for Git repo")?;
            git2::Repository::init_bare(&repo_path).context("initializing empty git repository")?
        };

        let mut remote = if let Ok(remote) = repo.find_remote("origin") {
            if remote.url() != Some(GLIBC_GIT_URL) {
                warn!(&self.logger, "updating origin URL to {}", GLIBC_GIT_URL);
                repo.remote_set_url("origin", GLIBC_GIT_URL)?;
            }

            remote
        } else {
            warn!(&self.logger, "setting origin remote to {}", GLIBC_GIT_URL);
            repo.remote("origin", GLIBC_GIT_URL)?
        };

        warn!(&self.logger, "connecting to {}", GLIBC_GIT_URL);
        remote.connect(git2::Direction::Fetch)?;

        let refs = remote
            .list()?
            .into_iter()
            .filter(|head| head.name().starts_with("refs/tags/glibc-"))
            .filter(|head| !head.name().contains('^'))
            .filter(|head| repo.refname_to_id(head.name()) != Ok(head.oid()))
            .map(|head| format!("+{}:{}", head.name(), head.name()))
            .collect::<Vec<_>>();

        let mut last_percent_objects = 0;
        let mut last_percent_deltas = 0;
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.transfer_progress(|progress| {
            let objects_percent = ((progress.received_objects() as f64
                / progress.total_objects() as f64)
                * 100.0) as usize;

            if objects_percent > last_percent_objects {
                warn!(
                    &self.logger,
                    "received {}% of objects ({}/{})",
                    objects_percent,
                    progress.received_objects(),
                    progress.total_objects()
                );
            }

            if progress.total_deltas() > 0 {
                let deltas_percent = ((progress.indexed_deltas() as f64
                    / progress.total_deltas() as f64)
                    * 100.0) as usize;

                if deltas_percent > last_percent_deltas {
                    warn!(
                        &self.logger,
                        "indexed {}% of deltas ({}/{})",
                        deltas_percent,
                        progress.indexed_deltas(),
                        progress.total_deltas()
                    );
                }

                last_percent_deltas = deltas_percent;
            }

            last_percent_objects = objects_percent;

            true
        });

        let mut options = git2::FetchOptions::new();
        options.remote_callbacks(callbacks);

        if !refs.is_empty() {
            warn!(&self.logger, "fetching {} refs", refs.len());
            remote.fetch(&refs, Some(&mut options), None)?;
        } else {
            warn!(&self.logger, "repo up to date");
        }

        Ok(())
    }

    pub async fn docker_image_glibc(&self, dest_dir: Option<&Path>) -> Result<String> {
        let docker = self.docker_client()?;

        let image_id =
            crate::docker::build_image_glibc(&self.logger, &docker, &self.cache_dir).await?;

        if let Some(dest_path) = dest_dir {
            let (in_size, out_size) =
                crate::docker::export_image_to_tar_zst(&docker, &image_id, dest_path)
                    .await
                    .context("exporting Docker image to file")?;
            warn!(
                &self.logger,
                "wrote {}; compressed {} -> {} bytes",
                dest_path.display(),
                in_size,
                out_size
            );
        }

        Ok(image_id)
    }

    /// Write glibc ABI metadata to a tar.zst file.
    pub async fn glibc_abis(&self, dest_path: &Path, image_path: Option<&Path>) -> Result<()> {
        let docker = self.docker_client()?;

        let image_id = if let Some(image_path) = image_path {
            let fh = std::fs::File::open(image_path).context("opening image archive")?;

            crate::docker::load_image_tar_zst(&self.logger, &docker, fh)
                .await
                .context("loading Docker image")?
        } else {
            self.docker_image_glibc(None)
                .await
                .context("building glibc Docker image")?
        };

        let abis = crate::docker::glibc_abis(&self.logger, &docker, &image_id)
            .await
            .context("collecting glibc ABIs")?;

        let tar_data = crate::tar::TarBuilder::from(abis).as_vec()?;
        let tar_data = zstd::encode_all(Cursor::new(tar_data), ZSTD_COMPRESSION_LEVEL)?;
        std::fs::write(dest_path, &tar_data).context("writing glibc ABI tar.zst file")?;

        Ok(())
    }

    /// Build a single configuration of glibc.
    pub async fn glibc_build_single(
        &self,
        dest_dir: &Path,
        compiler: &str,
        glibc: &str,
        image_path: Option<&Path>,
    ) -> Result<()> {
        let docker = self.docker_client()?;

        let image_id = if let Some(image_path) = image_path {
            let fh = std::fs::File::open(image_path).context("opening image archive")?;

            crate::docker::load_image_tar_zst(&self.logger, &docker, fh)
                .await
                .context("loading Docker image")?
        } else {
            self.docker_image_glibc(None)
                .await
                .context("building glibc Docker image")?
        };

        let tar_data =
            crate::docker::glibc_build_single(&self.logger, &docker, &image_id, compiler, glibc)
                .await
                .context("building glibc in container")?;
        let tar_data = zstd::encode_all(Cursor::new(tar_data), ZSTD_COMPRESSION_LEVEL)?;
        std::fs::write(dest_dir.join(format!("glibc-{}.tar.zst", glibc)), &tar_data)?;

        Ok(())
    }

    /// Unify glibc builds from source archives.
    pub fn glibc_unify(
        &self,
        source_archives: &[&Path],
        dest_dir: Option<&Path>,
        dest_tar_zst: Option<&Path>,
        headers_only: bool,
    ) -> Result<()> {
        let temp_dir = tempfile::Builder::new().prefix("pclang-").tempdir()?;
        let temp_root = temp_dir.path();
        let source_dir = temp_root.join("glibcs");
        let temp_unified_dir = temp_root.join("unified");

        let dest_dir = dest_dir.unwrap_or(&temp_unified_dir);

        for source_archive in source_archives {
            warn!(&self.logger, "extracting {}", source_archive.display());
            let fh = std::fs::File::open(source_archive).context("opening glibc source archive")?;
            let stream = zstd::stream::Decoder::new(fh).context("creating zstd decompressor")?;
            let mut archive = tar::Archive::new(stream);

            archive
                .unpack(&source_dir)
                .context("extracting tar archive")?;
        }

        crate::glibc::unify_glibc(&self.logger, &source_dir, dest_dir, headers_only)
            .context("unifying glibc")?;

        if let Some(dest_tar_zst) = dest_tar_zst {
            let tar_data = tar_from_directory(&self.logger, dest_dir, Some(Path::new("glibcs")))
                .context("creating tar of unified glibc")?;

            let fh = std::fs::File::create(dest_tar_zst)?;
            zstd::stream::copy_encode(Cursor::new(tar_data), fh, ZSTD_COMPRESSION_LEVEL)?;
        }

        Ok(())
    }
}
