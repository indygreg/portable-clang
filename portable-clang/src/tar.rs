// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    anyhow::{anyhow, Context, Result},
    hyper::Body,
    slog::{warn, Logger},
    std::{io::Cursor, path::Path},
    tugger_file_manifest::{is_executable, FileEntry, FileManifest},
};

#[cfg(target_family = "unix")]

/// Obtain contents of a GNU tar archive from a source directory.
pub fn tar_from_directory(
    logger: &Logger,
    path: impl AsRef<Path>,
    path_prefix: Option<&Path>,
) -> Result<Vec<u8>> {
    let root_dir = path.as_ref();
    let path_prefix = path_prefix.map(|x| x.to_path_buf());

    let mut builder = tar::Builder::new(vec![]);

    for entry in walkdir::WalkDir::new(root_dir)
        .follow_links(false)
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
    {
        let entry = entry?;

        let archive_path = entry.path().strip_prefix(root_dir)?;

        let archive_path = if let Some(prefix) = &path_prefix {
            prefix.join(archive_path)
        } else {
            archive_path.to_path_buf()
        };

        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            continue;
        }

        warn!(logger, "adding {} to tar archive", archive_path.display());

        let mut header = tar::Header::new_gnu();

        header.set_mode(if is_executable(&metadata) {
            0o755
        } else {
            0o644
        });

        header.set_mtime(1609502400);
        header.set_uid(0);
        header.set_gid(0);

        let data = if metadata.file_type().is_symlink() {
            let link_name = std::fs::read_link(entry.path()).context("reading link")?;
            header
                .set_link_name(&link_name)
                .context("setting link name")?;
            header.set_entry_type(tar::EntryType::Symlink);

            vec![]
        } else {
            header.set_entry_type(tar::EntryType::Regular);

            std::fs::read(entry.path())?
        };

        header.set_size(data.len() as _);
        builder.append_data(&mut header, archive_path, Cursor::new(data))?;
    }

    builder.finish()?;

    Ok(builder.into_inner()?)
}

#[derive(Clone, Debug, Default)]
pub struct TarBuilder {
    pub(crate) files: FileManifest,
}

impl From<FileManifest> for TarBuilder {
    fn from(files: FileManifest) -> Self {
        Self { files }
    }
}

impl TarBuilder {
    /// Define content for `Dockerfile`.
    pub fn add_dockerfile_data(&mut self, data: &[u8]) -> Result<()> {
        self.files
            .add_file_entry("Dockerfile", FileEntry::new_from_data(data, false))?;

        Ok(())
    }

    /// Add a path on the filesystem to a path prefix in the archive.
    pub fn add_path_with_prefix(
        &mut self,
        logger: &Logger,
        path: impl AsRef<Path>,
        prefix: impl AsRef<Path>,
    ) -> Result<()> {
        let path = path.as_ref();

        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow!("could not resolve file name"))?;

        let entry = FileEntry::try_from(path)?;
        let archive_path = prefix.as_ref().join(file_name);

        warn!(
            logger,
            "adding {} from {}",
            archive_path.display(),
            path.display()
        );
        self.files
            .add_file_entry(&archive_path, entry)
            .context("adding support file to tar archive")
    }

    /// Obtain an uncompressed tarball of content.
    pub fn as_vec(&self) -> Result<Vec<u8>> {
        let mut builder = tar::Builder::new(vec![]);

        for (path, entry) in self.files.iter_entries() {
            let data = entry.resolve_content()?;
            let mut header = tar::Header::new_gnu();
            header.set_mode(if entry.is_executable() { 0o755 } else { 0o644 });
            header.set_size(data.len() as _);

            builder.append_data(&mut header, &path, Cursor::new(data))?;
        }

        builder.finish()?;

        Ok(builder.into_inner()?)
    }

    /// Obtain a [Body] containing the tar archive content.
    pub fn as_body(&self) -> Result<Body> {
        Ok(Body::from(self.as_vec()?))
    }
}
