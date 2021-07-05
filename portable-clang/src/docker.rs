// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/*! Docker functionality. */

use {
    crate::tar::{tar_from_directory, TarBuilder},
    anyhow::{anyhow, Context, Result},
    bollard::{
        container::{
            Config as ContainerConfig, CreateContainerOptions, LogsOptions, StartContainerOptions,
        },
        image::{BuildImageOptions, ImportImageOptions},
        models::HostConfig,
        Docker,
    },
    futures_util::stream::TryStreamExt,
    hyper::body::Body,
    indoc::indoc,
    slog::{warn, Logger},
    std::{
        collections::HashMap,
        io::{Cursor, Read, Write},
        path::Path,
    },
    tugger_file_manifest::{FileEntry, FileManifest},
};

#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;

pub const ZSTD_COMPRESSION_LEVEL: i32 = 8;

const DEBIAN_JESSIE_HEADER: &str = indoc! {r#"
    FROM debian@sha256:32ad5050caffb2c7e969dac873bce2c370015c2256ff984b70c1c08b3a2816a0
    MAINTAINER Gregory Szorc <gregory.szorc@gmail.com>

    RUN groupadd -g 1000 build && \
        useradd -u 1000 -g 1000 -d /build -s /bin/bash -m build && \
        chown -R build:build /build

    ENV HOME=/build \
        SHELL=/bin/bash \
        USER=build \
        LOGNAME=build \
        HOSTNAME=builder \
        DEBIAN_FRONTEND=noninteractive

    CMD ["/bin/bash", "--login"]
    WORKDIR '/build'

    RUN for s in debian_jessie debian_jessie-updates debian-security_jessie/updates; do \
          echo "deb http://snapshot.debian.org/archive/${s%_*}/20211107T145307Z/ ${s#*_} main"; \
        done > /etc/apt/sources.list && \
        ( echo 'quiet "true";'; \
          echo 'APT::Get::Assume-Yes "true";'; \
          echo 'APT::Install-Recommends "false";'; \
          echo 'Acquire::Check-Valid-Until "false";'; \
          echo 'Acquire::Retries "5";'; \
        ) > /etc/apt/apt.conf.d/99portable-clang

    RUN apt-get update
"#};

const DEBIAN_JESSIE_FOOTER: &str = indoc! {r#"
    COPY files/* /build/
    COPY scripts/ /usr/bin/
    USER build:build
"#};

const DEBIAN_BULLSEYE_HEADER: &str = indoc! {r#"
    FROM debian@sha256:4d6ab716de467aad58e91b1b720f0badd7478847ec7a18f66027d0f8a329a43c
    MAINTAINER Gregory Szorc <gregory.szorc@gmail.com>

    RUN groupadd -g 1000 build && \
        useradd -u 1000 -g 1000 -d /build -s /bin/bash -m build && \
        chown -R build:build /build

    ENV HOME=/build \
        SHELL=/bin/bash \
        USER=build \
        LOGNAME=build \
        HOSTNAME=builder \
        DEBIAN_FRONTEND=noninteractive

    CMD ["/bin/bash", "--login"]
    WORKDIR '/build'

    RUN for s in debian_bullseye debian_bullseye-updates; do \
          echo "deb http://snapshot.debian.org/archive/${s%_*}/20211107T145307Z/ ${s#*_} main"; \
        done > /etc/apt/sources.list && \
        ( echo 'quiet "true";'; \
          echo 'APT::Get::Assume-Yes "true";'; \
          echo 'APT::Install-Recommends "false";'; \
          echo 'Acquire::Check-Valid-Until "false";'; \
          echo 'Acquire::Retries "5";'; \
        ) > /etc/apt/apt.conf.d/99portable-clang

    RUN apt-get update
"#};

const CLANG_DOCKERFILE: &str = indoc! {r#"
    RUN mkdir /toolchains && chown build:build /toolchains
    RUN apt-get install \
        ca-certificates \
        libc6-dev \
        patch \
        tar \
        xz-utils \
        unzip \
        zlib1g-dev
"#};

const GCC_DOCKERFILE: &str = indoc! {r#"
    RUN mkdir /toolchains && chown build:build /toolchains
    RUN apt-get install \
        autoconf \
        automake \
        bison \
        build-essential \
        ca-certificates \
        gawk \
        gcc \
        gcc-multilib \
        libtool \
        make \
        tar \
        texinfo \
        xz-utils \
        unzip
"#};

const GLIBC_DOCKERFILE: &str = indoc! {r#"
    RUN mkdir /toolchains && chown build:build /toolchains
    RUN apt-get install \
        autoconf \
        automake \
        bison \
        build-essential \
        ca-certificates \
        flex \
        gawk \
        git \
        procps \
        python3 \
        rsync \
        texinfo \
        watch

    # We do this one as a one-off because it takes a while to run and caching the layer is
    # useful for iterative development.
    COPY scripts/docker-glibc-init.sh /usr/bin/
    COPY files/build-many-glibcs* /build/
    RUN /usr/bin/docker-glibc-init.sh

    COPY files/* /build/
    COPY scripts/* /usr/bin/

    USER build:build
"#};

pub fn docker_client() -> Result<Docker> {
    Ok(Docker::connect_with_socket(
        "unix:///var/run/docker.sock",
        600,
        bollard::API_DEFAULT_VERSION,
    )?)
}

/// Build a Docker image with context.
pub async fn build_image(
    logger: &Logger,
    docker: &Docker,
    options: BuildImageOptions<String>,
    body: Body,
) -> Result<String> {
    let mut stream = docker.build_image(options, None, Some(body));

    while let Some(info) = stream.try_next().await? {
        if let Some(stream) = info.stream {
            for part in stream.split('\n').filter(|s| !s.is_empty()) {
                warn!(logger, "{}", part);
            }
        } else if let Some(status) = info.status {
            if let Some(progress) = info.progress {
                warn!(logger, "{} {}", status, progress);
            } else {
                warn!(logger, "{}", status);
            }
        } else if let Some(image_id) = info.aux {
            return image_id.id.ok_or_else(|| anyhow!("image ID not set"));
        }
    }

    Err(anyhow!("error building image"))
}

/// Load image tar data.
pub async fn load_image_tar(logger: &Logger, docker: &Docker, tar_data: Vec<u8>) -> Result<String> {
    let options = ImportImageOptions::default();
    let mut stream = docker.import_image(options, Body::from(tar_data), None);

    while let Some(info) = stream.try_next().await? {
        if let Some(stream) = info.stream {
            for part in stream.split('\n').filter(|s| !s.is_empty()) {
                warn!(logger, "{}", part);

                // For some reason we don't get the image ID reported in any aux responses.
                // So parse it from stdout. This is extremely hacky.
                if let Some((_, id)) = part.split_once("Loaded image ID: ") {
                    return Ok(id.to_string());
                }
            }
        } else if let Some(status) = info.status {
            if let Some(progress) = info.progress {
                warn!(logger, "{} {}", status, progress);
            } else {
                warn!(logger, "{}", status);
            }
        } else if let Some(image_id) = info.aux {
            return image_id.id.ok_or_else(|| anyhow!("image ID not set"));
        }
    }

    Err(anyhow!("error loading image"))
}

/// Load a tar.zst file into Docker and return the image id.
pub async fn load_image_tar_zst(
    logger: &Logger,
    docker: &Docker,
    reader: impl Read,
) -> Result<String> {
    let tar_data = zstd::decode_all(reader).context("zstd decompressing image data")?;

    // In CI we see JSON decode errors intermittently. The root cause is unknown.
    // https://github.com/fussybeaver/bollard/issues/171. We retry the operation
    // multiple times as a workaround.
    for attempt in 0..5 {
        match load_image_tar(logger, docker, tar_data.clone()).await {
            Ok(image) => {
                return Ok(image);
            }
            Err(e) => {
                warn!(logger, "attempt #{}: image load failed: {:?}", attempt, e);
            }
        }
    }

    Err(anyhow!("image load failed multiple times"))
}

async fn run_and_log_container(
    logger: &Logger,
    docker: &Docker,
    options: CreateContainerOptions<String>,
    config: ContainerConfig<String>,
) -> Result<()> {
    let response = docker
        .create_container(Some(options), config)
        .await
        .context("creating Docker container")?;
    let container_id = response.id;

    let options = StartContainerOptions::<String>::default();
    docker
        .start_container(&container_id, Some(options))
        .await
        .context("starting Docker container")?;

    let options = LogsOptions::<String> {
        follow: true,
        stdout: true,
        stderr: true,
        ..Default::default()
    };
    let mut stream = docker.logs(&container_id, Some(options));

    while let Some(output) = stream.try_next().await? {
        for line in output.to_string().split('\n').filter(|x| !x.is_empty()) {
            warn!(logger, "{}", line);
        }
    }

    Ok(())
}

fn derive_dockerfile_version_envs() -> String {
    let parts = crate::downloads::DOWNLOADS
        .values()
        .map(|record| {
            format!(
                "{}_VERSION={}",
                record.name.to_uppercase().replace('-', "_"),
                record.version
            )
        })
        .collect::<Vec<_>>();

    format!("ENV {}", parts.join("\\\n    "))
}

/// Build the Docker image for building clang.
pub async fn build_image_clang(
    logger: &Logger,
    docker: &Docker,
    cache_path: impl AsRef<Path>,
) -> Result<String> {
    let cache_path = cache_path.as_ref();

    let mut tar = TarBuilder::default();

    for path in crate::downloads::fetch_llvm_sources(logger, cache_path)
        .context("fetching LLVM sources")?
        .into_iter()
        .chain(
            crate::downloads::fetch_linux_x86_64_support(logger, cache_path)
                .context("fetching support files")?
                .into_iter(),
        )
    {
        tar.add_path_with_prefix(logger, path, "files")?;
    }

    tar.files.add_file_entry(
        "scripts/docker-clang-build.sh",
        FileEntry::new_from_data(
            include_bytes!("scripts/docker-clang-build.sh").to_vec(),
            true,
        ),
    )?;
    tar.files.add_file_entry(
        "scripts/docker-extract-sccache.sh",
        FileEntry::new_from_data(
            include_bytes!("scripts/docker-extract-sccache.sh").to_vec(),
            true,
        ),
    )?;

    let dockerfile = format!(
        "{}\n{}\n{}\n{}",
        DEBIAN_JESSIE_HEADER,
        CLANG_DOCKERFILE,
        derive_dockerfile_version_envs(),
        DEBIAN_JESSIE_FOOTER
    );
    tar.add_dockerfile_data(dockerfile.as_bytes())?;

    let body = tar.as_body().context("building tar content")?;

    let options = BuildImageOptions::<String> {
        t: "portable-clang:clang".to_string(),
        ..Default::default()
    };

    build_image(logger, docker, options, body).await
}

/// Build a Docker image for building GCC.
pub async fn build_image_gcc(
    logger: &Logger,
    docker: &Docker,
    cache_dir: impl AsRef<Path>,
) -> Result<String> {
    let cache_dir = cache_dir.as_ref();

    let mut tar = TarBuilder::default();

    for path in crate::downloads::fetch_gcc_sources(logger, cache_dir)
        .context("fetching GCC sources")?
        .into_iter()
        .chain(
            crate::downloads::fetch_linux_x86_64_support(logger, cache_dir)
                .context("fetching support files")?
                .into_iter(),
        )
    {
        tar.add_path_with_prefix(logger, path, "files")?;
    }

    tar.files.add_file_entry(
        "scripts/docker-gcc-build.sh",
        FileEntry::new_from_data(include_bytes!("scripts/docker-gcc-build.sh").to_vec(), true),
    )?;
    tar.files.add_file_entry(
        "scripts/docker-extract-sccache.sh",
        FileEntry::new_from_data(
            include_bytes!("scripts/docker-extract-sccache.sh").to_vec(),
            true,
        ),
    )?;

    let dockerfile = format!(
        "{}\n{}\n{}\n{}",
        DEBIAN_JESSIE_HEADER,
        GCC_DOCKERFILE,
        derive_dockerfile_version_envs(),
        DEBIAN_JESSIE_FOOTER
    );
    tar.add_dockerfile_data(dockerfile.as_bytes())?;

    let body = tar.as_body().context("building tar content")?;

    let options = BuildImageOptions::<String> {
        t: "portable-clang:gcc".to_string(),
        ..Default::default()
    };

    build_image(logger, docker, options, body).await
}

/// Build a Docker image for building glibc.
pub async fn build_image_glibc(
    logger: &Logger,
    docker: &Docker,
    cache_dir: impl AsRef<Path>,
) -> Result<String> {
    let cache_dir = cache_dir.as_ref();

    let mut tar = TarBuilder::default();

    for path in crate::downloads::fetch_linux_x86_64_support(logger, cache_dir)
        .context("fetching support files")?
        .into_iter()
    {
        tar.add_path_with_prefix(logger, path, "files")?;
    }

    tar.files.add_file_entry(
        "files/build-many-glibcs.py",
        FileEntry::new_from_data(include_bytes!("files/build-many-glibcs.py").to_vec(), true),
    )?;
    tar.files.add_file_entry(
        "files/build-many-glibcs-sccache.patch",
        FileEntry::new_from_data(
            include_bytes!("files/build-many-glibcs-sccache.patch").to_vec(),
            false,
        ),
    )?;
    tar.files.add_file_entry(
        "scripts/docker-glibc-build.sh",
        FileEntry::new_from_data(
            include_bytes!("scripts/docker-glibc-build.sh").to_vec(),
            true,
        ),
    )?;
    tar.files.add_file_entry(
        "scripts/docker-glibc-init.sh",
        FileEntry::new_from_data(
            include_bytes!("scripts/docker-glibc-init.sh").to_vec(),
            true,
        ),
    )?;
    tar.files.add_file_entry(
        "scripts/docker-extract-sccache.sh",
        FileEntry::new_from_data(
            include_bytes!("scripts/docker-extract-sccache.sh").to_vec(),
            true,
        ),
    )?;
    tar.files.add_file_entry(
        "scripts/docker-glibc-collect-abi.py",
        FileEntry::new_from_data(
            include_bytes!("scripts/docker-glibc-collect-abi.py").to_vec(),
            true,
        ),
    )?;

    let dockerfile = format!(
        "{}\n{}\n{}",
        DEBIAN_BULLSEYE_HEADER,
        GLIBC_DOCKERFILE,
        derive_dockerfile_version_envs(),
    );
    tar.add_dockerfile_data(dockerfile.as_bytes())?;

    let body = tar.as_body().context("building tar content")?;

    let options = BuildImageOptions::<String> {
        t: "portable-clang:glibc".to_string(),
        ..Default::default()
    };

    build_image(logger, docker, options, body).await
}

/// Export a Docker image specified by its ID to a zstd compressed tar file at the given path.
pub async fn export_image_to_tar_zst(
    docker: &Docker,
    image_id: &str,
    dest_path: impl AsRef<Path>,
) -> Result<(u64, u64)> {
    let dest_path = dest_path.as_ref();

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).context("creating parent directory")?;
    }

    let fh = std::fs::File::create(dest_path).context("opening file for writing")?;
    let mut cctx =
        zstd::Encoder::new(fh, ZSTD_COMPRESSION_LEVEL).context("creating zstd encoder")?;

    let mut stream = docker.export_image(image_id);
    let mut in_size = 0;

    while let Some(data) = stream.try_next().await? {
        in_size += data.len() as u64;
        cctx.write_all(data.as_ref())
            .context("writing data to zstd")?;
    }

    let fh = cctx.finish().context("finishing zstd encoder")?;
    let out_size = fh.metadata().context("reading image file metadata")?.len();

    Ok((in_size, out_size))
}

fn add_container_envs(config: &mut ContainerConfig<String>) -> Result<()> {
    let env = config.env.get_or_insert(vec![]);

    // sccache speeds up builds considerably. So build with high parallelism.
    env.push(format!("PARALLEL={}", num_cpus::get() * 2));

    let mut have_remote_sccache = false;

    let mut envs: HashMap<String, String> = HashMap::from_iter(std::env::vars());

    // Supplement environment variables with set from a config file.
    if let Some(home) = dirs::home_dir() {
        let extra_path = home.join(".pclang-docker-env");

        if extra_path.exists() {
            let data = std::fs::read_to_string(&extra_path)
                .context("reading extra environment variables file")?;

            for line in data
                .split('\n')
                .filter(|x| !x.starts_with('#') && x.contains('='))
            {
                let (key, value) = line.split_once('=').expect("verified = is present above");
                envs.insert(key.to_string(), value.to_string());
            }
        }
    }

    for key in [
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "SCCACHE_BUCKET",
    ] {
        if let Some(value) = envs.get(key) {
            env.push(format!("{}={}", key, value));

            if key == "SCCACHE_BUCKET" {
                have_remote_sccache = true;
            }
        }
    }

    if have_remote_sccache {
        env.push("SCCACHE_S3_USE_SSL=1".into());
        env.push("SCCACHE_IDLE_TIMEOUT=0".into());
    } else {
        env.push("SCCACHE_DIR=/sccache".into());
    }

    Ok(())
}

/// Bootstrap the GCC toolchain.
///
/// We produce binutils + gcc artifacts that are used to build clang.
pub async fn bootstrap_gcc(
    logger: &Logger,
    docker: &Docker,
    image_id: &str,
    cache_dir: impl AsRef<Path>,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let cache_dir = cache_dir.as_ref();
    let sccache_dir = cache_dir.join("sccache");
    std::fs::create_dir_all(&sccache_dir)?;

    let temp_dir = tempfile::Builder::new().prefix("pclang-").tempdir()?;
    let out_dir = temp_dir.path();
    let mut permissions = out_dir.metadata()?.permissions();
    permissions.set_mode(0o0777);
    std::fs::set_permissions(&out_dir, permissions)
        .context("setting temp directory permissions")?;

    let options = CreateContainerOptions::<String>::default();

    let mut config = ContainerConfig::<String> {
        attach_stdin: Some(false),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        cmd: Some(vec!["/usr/bin/docker-gcc-build.sh".into()]),
        image: Some(image_id.into()),
        host_config: Some(HostConfig {
            auto_remove: Some(true),
            binds: Some(vec![
                format!("{}:/out", out_dir.display()),
                format!("{}:/sccache", sccache_dir.display()),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    add_container_envs(&mut config)?;

    run_and_log_container(logger, docker, options, config)
        .await
        .context("running container")?;

    let binutils_tar = tar_from_directory(
        logger,
        out_dir.join("binutils"),
        Some(Path::new("binutils")),
    )?;
    let gcc_tar = tar_from_directory(logger, out_dir.join("gcc"), Some(Path::new("gcc")))?;

    let binutils_tar_zst = zstd::encode_all(Cursor::new(binutils_tar), ZSTD_COMPRESSION_LEVEL)?;
    let gcc_tar_zst = zstd::encode_all(Cursor::new(gcc_tar), ZSTD_COMPRESSION_LEVEL)?;

    Ok((binutils_tar_zst, gcc_tar_zst))
}

pub async fn bootstrap_clang(
    logger: &Logger,
    docker: &Docker,
    image_id: &str,
    binutils_tar: &[u8],
    gcc_tar: &[u8],
    cache_dir: impl AsRef<Path>,
) -> Result<Vec<u8>> {
    let cache_dir = cache_dir.as_ref();
    let sccache_dir = cache_dir.join("sccache");
    std::fs::create_dir_all(&sccache_dir).context("creating sccache cache directory")?;

    let temp_dir = tempfile::Builder::new().prefix("pclang-").tempdir()?;
    let temp_dir_path = temp_dir.path();

    let in_dir = temp_dir_path.join("inputs");
    std::fs::create_dir_all(&in_dir).context("creating inputs directory")?;

    let fh = std::fs::File::create(in_dir.join("binutils.tar"))?;
    zstd::stream::copy_decode(binutils_tar, fh).context("zstd decompressing binutils")?;
    let fh = std::fs::File::create(in_dir.join("gcc.tar"))?;
    zstd::stream::copy_decode(gcc_tar, fh).context("zstd decompressing gcc")?;

    let out_dir = temp_dir_path.join("out");
    std::fs::create_dir_all(&out_dir).context("creating artifact outputs directory")?;
    let mut permissions = out_dir
        .metadata()
        .context("retrieving outputs directory metadata")?
        .permissions();
    permissions.set_mode(0o0777);
    std::fs::set_permissions(&out_dir, permissions)
        .context("setting temp directory permissions")?;

    let options = CreateContainerOptions::<String>::default();

    let mut config = ContainerConfig::<String> {
        attach_stdin: Some(false),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        cmd: Some(vec!["/usr/bin/docker-clang-build.sh".into()]),
        image: Some(image_id.into()),
        host_config: Some(HostConfig {
            auto_remove: Some(true),
            binds: Some(vec![
                format!("{}:/inputs", in_dir.display()),
                format!("{}:/out", out_dir.display()),
                format!("{}:/sccache", sccache_dir.display()),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    add_container_envs(&mut config)?;

    run_and_log_container(logger, docker, options, config)
        .await
        .context("running container")?;

    let clang_tar = tar_from_directory(logger, out_dir.join("clang"), Some(Path::new("clang")))?;
    warn!(logger, "compressing clang tarball");
    let clang_tar_zst = zstd::encode_all(Cursor::new(clang_tar), ZSTD_COMPRESSION_LEVEL)?;

    Ok(clang_tar_zst)
}

pub async fn glibc_abis(logger: &Logger, docker: &Docker, image_id: &str) -> Result<FileManifest> {
    let temp_dir = tempfile::Builder::new().prefix("pclang-").tempdir()?;
    let out_dir = temp_dir.path();
    let mut permissions = out_dir
        .metadata()
        .context("retrieving outputs directory metadata")?
        .permissions();
    permissions.set_mode(0o0777);
    std::fs::set_permissions(&out_dir, permissions)
        .context("setting temp directory permissions")?;

    let options = CreateContainerOptions::<String>::default();

    let config = ContainerConfig::<String> {
        attach_stdin: Some(false),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        cmd: Some(vec![
            "/usr/bin/docker-glibc-collect-abi.py".into(),
            "/build/src/glibc".into(),
            "/out".into(),
        ]),
        image: Some(image_id.into()),
        host_config: Some(HostConfig {
            auto_remove: Some(true),
            binds: Some(vec![format!("{}:/out", out_dir.display())]),
            ..Default::default()
        }),
        ..Default::default()
    };

    run_and_log_container(logger, docker, options, config)
        .await
        .context("running container")?;

    // The script deposited .json files for each ABI.
    let mut m = FileManifest::default();

    for entry in std::fs::read_dir(out_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|x| x.to_string_lossy()) != Some("json".into()) {
            continue;
        }

        m.add_path_memory(&path, out_dir)
            .context("adding JSON file to FileManifest")?;
    }

    Ok(m)
}

pub async fn glibc_build_single(
    logger: &Logger,
    docker: &Docker,
    image_id: &str,
    compiler: &str,
    glibc: &str,
) -> Result<Vec<u8>> {
    let temp_dir = tempfile::Builder::new().prefix("pclang-").tempdir()?;
    let out_dir = temp_dir.path();
    let mut permissions = out_dir
        .metadata()
        .context("retrieving outputs directory metadata")?
        .permissions();
    permissions.set_mode(0o0777);
    std::fs::set_permissions(&out_dir, permissions)
        .context("setting temp directory permissions")?;

    let options = CreateContainerOptions::<String>::default();

    let mut config = ContainerConfig::<String> {
        attach_stdin: Some(false),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        cmd: Some(vec![
            "/usr/bin/docker-glibc-build.sh".into(),
            compiler.into(),
            glibc.into(),
        ]),
        image: Some(image_id.into()),
        host_config: Some(HostConfig {
            auto_remove: Some(true),
            binds: Some(vec![format!("{}:/out", out_dir.display())]),
            ..Default::default()
        }),
        ..Default::default()
    };

    add_container_envs(&mut config)?;

    run_and_log_container(logger, docker, options, config)
        .await
        .context("running container")?;

    let glibc_path = out_dir.join(glibc);

    tar_from_directory(logger, glibc_path, Some(Path::new(glibc)))
}
