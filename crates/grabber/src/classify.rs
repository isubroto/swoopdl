//! Classify links before applying navigation depth or file filters.
use crate::LinkRole;
use url::Url;

pub fn link_role(url: &Url) -> LinkRole {
    let path = url.path().to_ascii_lowercase();
    if path.ends_with('/')
        || [".html", ".htm", ".xhtml", ".php", ".asp", ".aspx"]
            .iter()
            .any(|ext| path.ends_with(ext))
    {
        LinkRole::Navigation
    } else if [
        "mkv", "mp4", "webm", "avi", "mov", "mp3", "flac", "wav", "ogg", "zip", "rar", "7z", "pdf",
        "exe", "msi", "jpg", "jpeg", "png", "gif", "svg", "css", "js", "woff", "woff2", "ttf",
        "srt", "vtt",
    ]
    .iter()
    .any(|ext| path.ends_with(&format!(".{ext}")))
    {
        LinkRole::File
    } else {
        LinkRole::Unresolved
    }
}

pub fn is_navigation(mime: &str) -> bool {
    matches!(
        crate::media_type(mime).as_str(),
        "text/html" | "application/xhtml+xml" | "text/css"
    )
}

pub fn is_manifest(metadata: &crate::CandidateMetadata) -> bool {
    let path = metadata.final_url.path().to_ascii_lowercase();
    path.ends_with(".m3u8")
        || path.ends_with(".mpd")
        || matches!(
            crate::media_type(metadata.mime.as_deref().unwrap_or("")).as_str(),
            "application/vnd.apple.mpegurl" | "application/x-mpegurl" | "application/dash+xml"
        )
}
