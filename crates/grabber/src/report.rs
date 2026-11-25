//! Streaming exports deliberately omit all query values, fragments, and userinfo.
use crate::ProjectSnapshot;
use std::io::{self, Write};
use url::Url;
pub type ReportError = io::Error;
pub fn sanitized_url(url: &Url) -> String {
    let mut u = url.clone();
    let _ = u.set_username("");
    let _ = u.set_password(None);
    u.set_query(None);
    u.set_fragment(None);
    u.to_string()
}
pub fn write_report(state: &ProjectSnapshot, writer: &mut dyn Write) -> Result<(), ReportError> {
    writeln!(
        writer,
        "Swoopdl Site Grabber: {:?}; {} resources; {} decoded bytes",
        state.status,
        state.resources.len(),
        state.consumed_bytes
    )?;
    for resource in state.resources.values() {
        let status = if crate::policy::save_allowed(
            &state.config,
            &resource.url,
            resource.kind,
            &resource.mime,
        ) {
            "saved"
        } else {
            "type-filtered (traversal only)"
        };
        serde_json::to_writer(&mut *writer, &serde_json::json!({"url":sanitized_url(&resource.url),"status":status,"hash":resource.raw_hash})).map_err(io::Error::other)?;
        writeln!(writer)?;
    }
    for entry in &state.reports {
        serde_json::to_writer(
            &mut *writer,
            &serde_json::json!({"url":sanitized_url(&entry.url),"reason":entry.reason}),
        )
        .map_err(io::Error::other)?;
        writeln!(writer)?;
    }
    Ok(())
}
