// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/*! High level build logic. */

use {
    anyhow::{anyhow, Context, Result},
    slog::{warn, Logger},
    std::path::{Path, PathBuf},
};

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
            self.build_gcc(dest_dir).await?
        };

        let docker = self.docker_client()?;

        let image_id =
            crate::docker::build_image_clang(&self.logger, &docker, &self.cache_dir).await?;

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

    pub async fn build_gcc(&self, dest_dir: impl AsRef<Path>) -> Result<(Vec<u8>, Vec<u8>)> {
        let dest_dir = dest_dir.as_ref();

        std::fs::create_dir_all(dest_dir)?;

        let image_id =
            crate::docker::build_image_gcc(&self.logger, &self.docker_client()?, &self.cache_dir)
                .await?;

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
}
