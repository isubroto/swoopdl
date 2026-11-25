//! Hash names and destination containment, including Windows junctions.
use crate::UrlKey;
use std::{
    io,
    path::{Component, Path, PathBuf},
};
pub type PathError = io::Error;

pub fn local_path(key: &UrlKey, mime: &str) -> Result<PathBuf, PathError> {
    validate_hash(&key.0)?;
    let mime = crate::media_type(mime);
    let ext = match mime.as_str() {
        "text/html" | "application/xhtml+xml" => "html",
        "text/css" => "css",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/x-icon" => "ico",
        "font/woff" => "woff",
        "font/woff2" => "woff2",
        "font/ttf" => "ttf",
        "application/javascript" | "text/javascript" => "js",
        "application/pdf" => "pdf",
        _ => "bin",
    };
    Ok(format!("{}.{}", key.0, ext).into())
}
pub(crate) fn validate_hash(hash: &str) -> io::Result<()> {
    if hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(io::Error::other("Invalid content key"))
    }
}
fn check_existing(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err(io::Error::other(
                        "Reparse points are not allowed in capture paths",
                    ));
                }
            }
            if m.file_type().is_symlink() {
                return Err(io::Error::other(
                    "Symbolic links are not allowed in capture paths",
                ));
            }
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
pub fn checked_path(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(io::Error::other(
            "Capture path must be relative without traversal",
        ));
    }
    let root = std::path::absolute(root)?;
    check_existing(&root)?;
    // Resolve ancestors of the chosen root (macOS /var is a system symlink).
    // Links at the root itself or beneath it remain forbidden.
    let mut ancestor = root;
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .ok_or_else(|| io::Error::other("No capture parent directory"))?
                .to_os_string(),
        );
        if !ancestor.pop() {
            return Err(io::Error::other("No capture parent directory"));
        }
    }
    let mut current = ancestor.canonicalize()?;
    for component in suffix.into_iter().rev() {
        current.push(component);
    }
    for component in relative.components() {
        current.push(component);
        check_existing(&current)?;
    }
    Ok(current)
}
pub(crate) fn write(root: &Path, relative: &Path, body: &[u8]) -> io::Result<PathBuf> {
    let path = checked_path(root, relative)?;
    craft_atomic::atomic_write(&path, body)?;
    Ok(path)
}
