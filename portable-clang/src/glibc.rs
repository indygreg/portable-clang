// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    anyhow::{anyhow, Context, Result},
    sha2::Digest,
    slog::{info, warn, Logger},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
    },
};

#[cfg(target_family = "unix")]
use std::os::unix::fs::{symlink, PermissionsExt};

fn normalize_file(path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path)?;

    let mut p = metadata.permissions();

    p.set_mode(if p.mode() & 0o100 != 0 { 0o755 } else { 0o644 });

    std::fs::set_permissions(path, p).context("setting permissions")?;

    Ok(())
}

/// Unify directories containing glibc builds.
///
/// [source_dir] contains sub-directories containing individual builds of glibc.
///
/// We scan each individual glibc source directory and record the files that
/// we've seen.
///
/// The source directories and files are rematerialized in [dest_dir] except
/// that duplicate files are normalized to symlinks to files in a shared location.
/// This ensures that each unique file is written exactly once.
pub fn unify_glibc(
    logger: &Logger,
    source_dir: &Path,
    dest_dir: &Path,
    headers_only: bool,
) -> Result<()> {
    let mut input_dirs = vec![];

    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;

        if entry.metadata()?.is_dir() {
            input_dirs.push(entry.path());
        }
    }

    input_dirs.sort();

    let mut digests = BTreeMap::<String, BTreeSet<PathBuf>>::new();

    for input_dir in input_dirs {
        warn!(logger, "indexing {}", input_dir.display());

        for entry in walkdir::WalkDir::new(&input_dir) {
            let entry = entry?;

            let relative_path = entry.path().strip_prefix(source_dir)?;

            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                continue;
            }

            if headers_only {
                if let Some(ext) = relative_path.extension() {
                    if ext != "h" {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            let mut h = sha2::Sha256::new();
            // Feed the executable bit into the digest to distinguish between
            // output file modes.
            h.update(format!("{}", metadata.permissions().mode() & 0o100));
            h.update(&std::fs::read(entry.path())?);

            let digest = h.finalize();
            let sha256 = hex::encode(digest.as_slice());

            digests
                .entry(sha256)
                .or_default()
                .insert(relative_path.to_path_buf());
        }
    }

    let mut copy_count = 0;
    let mut dedupe_count = 0;
    let mut symlink_count = 0;

    for (digest, paths) in digests {
        // Exactly 1 file is a straight file copy.
        if paths.len() == 1 {
            copy_count += 1;

            let path = paths.iter().next().expect("set has exactly 1 element");

            let source_path = source_dir.join(path);
            let dest_path = dest_dir.join(path);

            std::fs::create_dir_all(
                dest_path
                    .parent()
                    .ok_or_else(|| anyhow!("failed to resolve parent directory"))?,
            )?;
            info!(
                logger,
                "copying {} -> {}",
                source_path.display(),
                dest_path.display()
            );
            std::fs::copy(&source_path, &dest_path).context("copying file")?;
            normalize_file(&dest_path)?;
        }
        // Multiple files is a symlink to a common file entry.
        else {
            dedupe_count += 1;

            let common_rel_path = PathBuf::from("common").join(&digest[0..2]).join(&digest);
            let common_path = dest_dir.join(&common_rel_path);

            std::fs::create_dir_all(
                common_path
                    .parent()
                    .ok_or_else(|| anyhow!("failed to resolve parent of common path"))?,
            )?;

            let mut paths_iter = paths.into_iter();
            let first_path = paths_iter
                .next()
                .ok_or_else(|| anyhow!("failed to get first path"))?;

            let source_path = source_dir.join(first_path);
            info!(
                logger,
                "copying {} -> {}",
                source_path.display(),
                common_path.display()
            );
            std::fs::copy(&source_path, &common_path)
                .context("copying source file to common path")?;
            normalize_file(&common_path)?;

            // Now install symlinks for remaining files.
            for path in paths_iter {
                symlink_count += 1;

                let symlink_source = dest_dir.join(&path);
                std::fs::create_dir_all(
                    symlink_source
                        .parent()
                        .ok_or_else(|| anyhow!("failed to get parent of symlink path"))?,
                )?;

                // The symlink target needs to be relative to the source path so the file layout
                // is portable.
                let mut symlink_target = PathBuf::new();
                for _ in 0..path.components().count() - 1 {
                    symlink_target.push("..");
                }
                let symlink_target = symlink_target.join(&common_rel_path);

                info!(
                    logger,
                    "symlinking {} -> {}",
                    symlink_source.display(),
                    symlink_target.display()
                );
                symlink(&symlink_target, &symlink_source).context("creating symlink")?;
            }
        }
    }

    warn!(
        logger,
        "copied {} files; symlinked {} files to {} common files",
        copy_count,
        symlink_count,
        dedupe_count
    );

    Ok(())
}
