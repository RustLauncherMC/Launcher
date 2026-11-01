//! Unpacking a Horizon build. Each archive holds one top folder (`horizon-linux-x86_64/…`);
//! its contents become the version's folder. Entries that would land outside it are refused.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Unpacks `archive` (`.tar.gz` or `.zip`) into `dest`, dropping the top folder.
pub fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    let name = archive.to_string_lossy();
    if name.ends_with(".tar.gz") {
        untar(archive, dest)
    } else if name.ends_with(".zip") {
        unzip(archive, dest)
    } else {
        bail!("unknown archive type: {name}")
    }
}

/// The path inside `dest` for an archive entry, without its top folder. `None` for the top
/// folder itself; an error for anything that would escape `dest`.
fn inside(dest: &Path, entry: &Path) -> Result<Option<PathBuf>> {
    let mut parts = Vec::new();
    for component in entry.components() {
        match component {
            Component::Normal(part) => parts.push(part),
            Component::CurDir => {}
            _ => bail!("unsafe path in the archive: {}", entry.display()),
        }
    }
    if parts.len() < 2 {
        return Ok(None);
    }
    Ok(Some(
        parts[1..]
            .iter()
            .fold(dest.to_path_buf(), |p, part| p.join(part)),
    ))
}

fn untar(archive: &Path, dest: &Path) -> Result<()> {
    let file = fs::File::open(archive)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries().context("unreadable archive")? {
        let mut entry = entry.context("unreadable archive")?;
        let entry_path = entry.path()?.into_owned();
        let Some(path) = inside(dest, &entry_path)? else {
            continue;
        };
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            fs::create_dir_all(&path)?;
        } else if kind.is_file() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            // `unpack` keeps the file's mode, so executables stay executable.
            entry.unpack(&path)?;
        } else {
            // Builds hold only folders and files; links could point outside `dest`.
            bail!("unexpected entry in the archive: {}", entry_path.display());
        }
    }
    Ok(())
}

fn unzip(archive: &Path, dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(fs::File::open(archive)?).context("unreadable archive")?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(name) = entry.enclosed_name() else {
            bail!("unsafe path in the archive: {}", entry.name());
        };
        let Some(path) = inside(dest, &name)? else {
            continue;
        };
        if entry.is_dir() {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&path)?;
        io::copy(&mut entry, &mut out)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn unpacks_a_tar_gz_without_its_top_folder() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("b.tar.gz");
        {
            let gz = flate2::write::GzEncoder::new(
                fs::File::create(&archive).unwrap(),
                flate2::Compression::fast(),
            );
            let mut tar = tar::Builder::new(gz);
            let mut header = tar::Header::new_gnu();
            header.set_size(2);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, "horizon-linux/horizon-client", &b"hi"[..])
                .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        let dest = dir.path().join("out");
        unpack(&archive, &dest).unwrap();
        let client = dest.join("horizon-client");
        assert_eq!(fs::read(&client).unwrap(), b"hi");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&client).unwrap().permissions().mode() & 0o111,
                0o111
            );
        }
    }

    #[test]
    fn unpacks_a_zip_without_its_top_folder() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("b.zip");
        {
            let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
            zip.start_file(
                "horizon-windows/bin/horizon-client.exe",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"exe").unwrap();
            zip.finish().unwrap();
        }
        let dest = dir.path().join("out");
        unpack(&archive, &dest).unwrap();
        assert_eq!(
            fs::read(dest.join("bin/horizon-client.exe")).unwrap(),
            b"exe"
        );
    }

    #[test]
    fn refuses_paths_that_escape() {
        let dest = Path::new("/d");
        assert!(inside(dest, Path::new("top/../../etc/passwd")).is_err());
        assert!(inside(dest, Path::new("/abs/x")).is_err());
        assert_eq!(inside(dest, Path::new("top")).unwrap(), None);
        assert_eq!(
            inside(dest, Path::new("top/a/b")).unwrap(),
            Some(PathBuf::from("/d/a/b"))
        );
    }
}
