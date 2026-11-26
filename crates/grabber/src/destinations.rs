//! Original file names with checked containment and explicit non-destructive collisions.
use crate::{
    CandidateMetadata, ProjectConfig,
    catalog::{CollisionPolicy, SaveLayout},
    paths,
};
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};

fn component(raw: &str) -> io::Result<String> {
    // Decode exactly once and reject remaining escapes to avoid platform-dependent double decoding.
    let bytes = raw.as_bytes();
    let mut decoded = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(io::Error::other("Invalid encoded filename"));
            }
            if !bytes[i + 1].is_ascii_hexdigit() || !bytes[i + 2].is_ascii_hexdigit() {
                return Err(io::Error::other("Invalid encoded filename"));
            }
            let v = u8::from_str_radix(&raw[i + 1..i + 3], 16).map_err(io::Error::other)?;
            decoded.push(v);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let name = String::from_utf8(decoded).map_err(io::Error::other)?;
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_uppercase();
    let port = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    let device = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || port.is_some_and(|s| {
        matches!(
            s,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    });
    if name.is_empty()
        || matches!(name.as_str(), "." | "..")
        || name.len() > 200
        || name.ends_with([' ', '.'])
        || device
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*%".contains(c))
    {
        return Err(io::Error::other("Unsafe file or folder name"));
    }
    Ok(name)
}

pub fn reserve(
    config: &ProjectConfig,
    metadata: &CandidateMetadata,
    layout: SaveLayout,
    collision: CollisionPolicy,
    reserved: &BTreeSet<PathBuf>,
) -> io::Result<Option<PathBuf>> {
    let name = component(&metadata.name)?;
    let mut relative = PathBuf::new();
    match layout {
        SaveLayout::Flat => {}
        SaveLayout::OriginalSubfolders => {
            if metadata.final_url.origin() != config.seed.origin() {
                return Err(io::Error::other("Original folders require the seed origin"));
            }
            let path = metadata
                .final_url
                .path()
                .strip_prefix(&config.path_prefix)
                .ok_or_else(|| io::Error::other("File is outside the source directory"))?;
            let mut parts = path.split('/').collect::<Vec<_>>();
            parts.pop();
            for p in parts {
                relative.push(component(p)?);
            }
        }
        SaveLayout::Category => {
            let mime = metadata.mime.as_deref().unwrap_or("");
            relative.push(if mime.starts_with("video/") {
                "Video"
            } else if mime.starts_with("audio/") {
                "Music"
            } else if mime.starts_with("image/") {
                "Images"
            } else if mime == "application/pdf" || mime.starts_with("text/") {
                "Documents"
            } else {
                "General"
            });
        }
    }
    relative.push(&name);
    for n in 0..100_000u32 {
        let mut candidate = relative.clone();
        if n > 0 {
            let file = Path::new(&name);
            let stem = file.file_stem().unwrap_or_default().to_string_lossy();
            let ext = file
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            candidate.set_file_name(format!("{stem} ({n}){ext}"));
        }
        let path = paths::checked_path(&config.destination, &candidate)?;
        let occupied = path.exists()
            || reserved.iter().any(|r| {
                r.to_string_lossy()
                    .eq_ignore_ascii_case(&path.to_string_lossy())
            })
            || sdl_engine::naming::part_path(&path).exists()
            || sdl_engine::state::state_path(&path).exists();
        if !occupied {
            return Ok(Some(path));
        }
        if collision == CollisionPolicy::Skip {
            return Ok(None);
        }
    }
    Err(io::Error::other("No collision-free filename is available"))
}
