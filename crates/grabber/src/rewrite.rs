//! Per-URL rewriting from immutable originals, then atomic generation publication.
use crate::{
    ProjectSnapshot, UrlKey,
    discover::{attributes, css_urls, html_base, settings, srcset},
    paths,
    store::ProjectStore,
};
use lol_html::{HtmlRewriter, doc_comments, doctype, element, html_content::ContentType, text};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};
use url::Url;
pub type ResourceMap = BTreeMap<UrlKey, PathBuf>;
pub type RewriteError = io::Error;
pub type PublishError = io::Error;
const OUTPUT_CAP: usize = 64 << 20;
const CSP: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; media-src 'self'; frame-src 'self'; script-src 'none'; connect-src 'none'; form-action 'none'; base-uri 'none'; object-src 'none'\">";

fn target(value: &str, base: &Url, map: &ResourceMap) -> String {
    if value.starts_with('#') || value.starts_with("data:") {
        return value.into();
    }
    let Ok(url) = base.join(value) else {
        return String::new();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return String::new();
    }
    if let Some(path) = map.get(&UrlKey::of(&url)) {
        let mut path = path.to_string_lossy().replace('\\', "/");
        if let Some(fragment) = url.fragment() {
            path.push('#');
            path.push_str(fragment);
        }
        path
    } else {
        url.to_string()
    }
}
fn css_string(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\x0c' | '<' => out.push_str(&format!("\\{:x} ", c as u32)),
            _ => out.push(c),
        }
    }
    out
}
fn rewrite_css(css: &str, base: &Url, map: &ResourceMap) -> io::Result<String> {
    let mut out = String::new();
    let mut previous = 0;
    for (span, value) in css_urls(css) {
        let quoted = css_string(&target(&value, base, map));
        let replacement = if css[span.clone()].starts_with(['"', '\'']) {
            format!("\"{quoted}\"")
        } else {
            format!("url(\"{quoted}\")")
        };
        if out
            .len()
            .saturating_add(span.start - previous)
            .saturating_add(replacement.len())
            > OUTPUT_CAP
        {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "Rewritten document exceeds output cap",
            ));
        }
        out.push_str(&css[previous..span.start]);
        out.push_str(&replacement);
        previous = span.end;
    }
    if out.len().saturating_add(css.len() - previous) > OUTPUT_CAP {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "Rewritten document exceeds output cap",
        ));
    }
    out.push_str(&css[previous..]);
    Ok(out)
}
pub fn rewrite_document(
    body: &[u8],
    mime: &str,
    base: &Url,
    map: &ResourceMap,
) -> Result<Vec<u8>, RewriteError> {
    if body.len() > crate::discover::PARSE_CAP {
        return Err(io::Error::other("Document exceeds parse limit"));
    }
    let mime = crate::media_type(mime);
    if mime == "image/svg+xml" {
        return sanitize_svg(body);
    }
    if mime == "text/css" {
        return Ok(rewrite_css(
            std::str::from_utf8(body).map_err(io::Error::other)?,
            base,
            map,
        )?
        .into_bytes());
    }
    if !matches!(mime.as_str(), "text/html" | "application/xhtml+xml") {
        return Ok(body.to_vec());
    }
    let base = html_base(body, base).map_err(io::Error::other)?;
    let mut output = CSP.as_bytes().to_vec();
    let mut overflow = false;
    let style = RefCell::new(String::new());
    let mut rewriter = HtmlRewriter::new(
        settings()
            .append_element_content_handler(element!("*", |el| {
                let tag = el.tag_name();
                if matches!(
                    tag.as_str(),
                    "script" | "base" | "object" | "embed" | "applet"
                ) {
                    el.remove();
                    return Ok(());
                }
                if tag == "form" {
                    el.remove_and_keep_content();
                }
                if tag == "meta" && el.get_attribute("http-equiv").is_some() {
                    el.remove();
                    return Ok(());
                }
                let names: Vec<_> = el.attributes().iter().map(|a| a.name()).collect();
                for name in names {
                    if name.starts_with("on")
                        || matches!(
                            name.as_str(),
                            "action"
                                | "formaction"
                                | "srcdoc"
                                | "ping"
                                | "integrity"
                                | "crossorigin"
                                | "autofocus"
                                | "background"
                                | "manifest"
                                | "data"
                        )
                    {
                        el.remove_attribute(&name);
                    }
                }
                if matches!(tag.as_str(), "button" | "input" | "select" | "textarea") {
                    el.set_attribute("disabled", "")?;
                }
                if tag == "iframe" {
                    el.set_attribute("sandbox", "allow-same-origin")?;
                }
                for (attr, _) in attributes(&tag, &el.get_attribute("rel").unwrap_or_default()) {
                    if let Some(value) = el.get_attribute(attr) {
                        el.set_attribute(attr, &target(&value, &base, map))?;
                    }
                }
                if tag == "link"
                    && !el
                        .get_attribute("rel")
                        .unwrap_or_default()
                        .split_whitespace()
                        .any(|r| matches!(r, "stylesheet" | "icon"))
                {
                    el.remove();
                }
                if let Some(value) = el.get_attribute("srcset") {
                    let candidates: Vec<_> = srcset(&value)
                        .into_iter()
                        .map(|(u, d)| format!("{} {}", target(&u, &base, map), d))
                        .collect();
                    el.set_attribute("srcset", &candidates.join(", "))?;
                }
                if let Some(value) = el.get_attribute("style") {
                    el.set_attribute("style", &rewrite_css(&value, &base, map)?)?;
                }
                Ok(())
            }))
            .append_element_content_handler(text!("style", |t| {
                style.borrow_mut().push_str(t.as_str());
                if t.last_in_text_node() {
                    t.replace(
                        &rewrite_css(&style.borrow(), &base, map)?,
                        ContentType::Html,
                    );
                    style.borrow_mut().clear();
                } else {
                    t.remove();
                }
                Ok(())
            })),
        |c: &[u8]| {
            if output.len().saturating_add(c.len()) <= OUTPUT_CAP {
                output.extend_from_slice(c);
            } else {
                overflow = true;
            }
        },
    );
    rewriter.write(body).map_err(io::Error::other)?;
    rewriter.end().map_err(io::Error::other)?;
    if overflow {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "Rewritten document exceeds output cap",
        ));
    }
    Ok(output)
}

fn sanitize_svg(body: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut parser = HtmlRewriter::new(
        settings()
            .append_document_content_handler(doctype!(|d| {
                d.remove();
                Ok(())
            }))
            .append_document_content_handler(doc_comments!(|c| {
                c.remove();
                Ok(())
            }))
            .append_element_content_handler(element!("*", |el| {
                let tag = el.tag_name().to_ascii_lowercase();
                if !matches!(
                    tag.as_str(),
                    "svg"
                        | "g"
                        | "defs"
                        | "path"
                        | "rect"
                        | "circle"
                        | "ellipse"
                        | "line"
                        | "polyline"
                        | "polygon"
                        | "text"
                        | "tspan"
                        | "title"
                        | "desc"
                        | "lineargradient"
                        | "radialgradient"
                        | "stop"
                        | "clippath"
                        | "mask"
                        | "pattern"
                        | "use"
                ) {
                    el.remove();
                    return Ok(());
                }
                let attrs: Vec<_> = el
                    .attributes()
                    .iter()
                    .map(|a| (a.name(), a.value()))
                    .collect();
                for (name, value) in attrs {
                    let name_lower = name.to_ascii_lowercase();
                    if name_lower.starts_with("on")
                        || matches!(name_lower.as_str(), "style" | "xml:base")
                        || (matches!(name_lower.as_str(), "href" | "xlink:href")
                            && !value.starts_with('#'))
                        || css_urls(&value)
                            .iter()
                            .any(|(_, url)| !url.starts_with('#'))
                    {
                        el.remove_attribute(&name);
                    }
                }
                Ok(())
            })),
        |c: &[u8]| output.extend_from_slice(c),
    );
    parser.write(body).map_err(io::Error::other)?;
    parser.end().map_err(io::Error::other)?;
    Ok(output)
}

pub fn resource_map(state: &ProjectSnapshot) -> io::Result<ResourceMap> {
    let mut map = ResourceMap::new();
    for (key, record) in &state.resources {
        if !crate::policy::save_allowed(&state.config, &record.url, record.kind, &record.mime) {
            continue;
        }
        let path = paths::local_path(key, &record.mime)?;
        map.insert(key.clone(), path.clone());
        map.insert(UrlKey::of(&record.final_url), path.clone());
        for alias in &record.aliases {
            map.insert(UrlKey::of(alias), path.clone());
        }
    }
    Ok(map)
}
pub fn publish_generation(
    store: &ProjectStore,
    state: &mut ProjectSnapshot,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<PathBuf, PublishError> {
    let check_cancel = || {
        if cancel.is_cancelled() {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Publication paused",
            ))
        } else {
            Ok(())
        }
    };
    check_cancel()?;
    let map = resource_map(state)?;
    let root = &state.config.destination;
    // A unique staging generation never replaces a previously published one.
    let mut number = state.generation.saturating_add(1);
    let (relative, directory) = loop {
        let relative = PathBuf::from("generations").join(format!("{number:020}"));
        let directory = paths::checked_path(root, &relative)?;
        match fs::create_dir(&directory) {
            Ok(()) => break (relative, directory),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                number = number
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("Generation overflow"))?
            }
            Err(e) => return Err(e),
        }
    };
    for (key, record) in &state.resources {
        check_cancel()?;
        if !map.contains_key(key) {
            continue;
        }
        let raw = store.verified_body(state, key)?;
        let mime = crate::media_type(&record.mime);
        let output = if matches!(
            mime.as_str(),
            "text/html" | "application/xhtml+xml" | "text/css" | "image/svg+xml"
        ) {
            rewrite_document(&raw, &mime, &record.final_url, &map)?
        } else {
            raw
        };
        let path = map
            .get(key)
            .ok_or_else(|| io::Error::other("Missing resource mapping"))?;
        paths::write(&directory, path, &output)?;
    }
    let seed = map
        .get(&UrlKey::of(&state.config.seed))
        .ok_or_else(|| io::Error::other("The seed page was not captured"))?;
    let entry = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Swoopdl offline capture</title><p><a href=\"{}/{}\">Open captured site</a></p>",
        relative.to_string_lossy().replace('\\', "/"),
        seed.to_string_lossy()
    );
    let report = paths::checked_path(&directory, Path::new("report.jsonl"))?;
    craft_atomic::atomic_write_with(&report, |writer| crate::report::write_report(state, writer))?;
    // Commit the durable generation marker before switching the human entry point.
    check_cancel()?;
    let old = state.generation;
    state.generation = number;
    let terminal = state.status;
    state.status = crate::ProjectStatus::Running;
    if let Err(e) = store.checkpoint(state) {
        state.generation = old;
        state.status = terminal;
        return Err(e);
    }
    state.status = terminal;
    check_cancel()?;
    paths::write(root, Path::new("index.html"), entry.as_bytes())?;
    store.checkpoint(state)?;
    Ok(root.join("index.html"))
}
