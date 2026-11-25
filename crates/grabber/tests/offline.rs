use sdl_grabber::{
    UrlKey,
    paths::local_path,
    rewrite::{ResourceMap, rewrite_document},
};
use url::Url;

#[test]
fn windows_case_and_reserved_names_do_not_collide() {
    let a = Url::parse("https://example.test/CON").unwrap();
    let b = Url::parse("https://example.test/con").unwrap();
    let pa = local_path(&UrlKey::of(&a), "text/html").unwrap();
    assert_ne!(pa, local_path(&UrlKey::of(&b), "text/html").unwrap());
    assert_eq!(pa.components().count(), 1);
    assert!(local_path(&UrlKey("../escape".into()), "text/html").is_err());
}

#[test]
fn uncaptured_link_keeps_absolute_target() {
    let base = Url::parse("https://example.test/docs/").unwrap();
    let body = br#"<base href="/else/"><a href="next#one">Next</a><script>alert(1)</script><img onerror="alert(1)" src="missing.png"><form action="/submit"><button>Go</button></form>"#;
    let out =
        String::from_utf8(rewrite_document(body, "text/html", &base, &ResourceMap::new()).unwrap())
            .unwrap();
    assert!(out.contains("https://example.test/else/next#one"));
    assert!(!out.contains("<script"));
    assert!(!out.contains("onerror"));
    assert!(!out.contains("<base"));
    assert!(!out.contains("<form"));
    assert!(out.contains("Content-Security-Policy"));
}

#[test]
fn same_raw_html_different_base_rewrites_separately() {
    let body = b"<a href='child'>Child</a>";
    let a = rewrite_document(
        body,
        "text/html",
        &Url::parse("https://example.test/a/").unwrap(),
        &ResourceMap::new(),
    )
    .unwrap();
    let b = rewrite_document(
        body,
        "text/html",
        &Url::parse("https://example.test/b/").unwrap(),
        &ResourceMap::new(),
    )
    .unwrap();
    assert_ne!(a, b);
}

#[test]
fn rewrite_output_expansion_is_capped() {
    let base = Url::parse(&format!("https://example.test/{}/", "a".repeat(8000))).unwrap();
    let body = "<a href='x'>Link</a>".repeat(10_000);
    assert!(rewrite_document(body.as_bytes(), "text/html", &base, &ResourceMap::new()).is_err());
}

#[test]
fn active_svg_cannot_execute_or_fetch_when_opened_directly() {
    let base = Url::parse("https://example.test/").unwrap();
    let body = br##"<svg xmlns="http://www.w3.org/2000/svg" onload="fetch('/leak')"><script>alert(1)</script><foreignObject><iframe src="https://other.test/"></iframe></foreignObject><image href="https://other.test/img"/><use href="#shape"/></svg>"##;
    let out = String::from_utf8(
        rewrite_document(body, "image/svg+xml", &base, &ResourceMap::new()).unwrap(),
    )
    .unwrap();
    assert!(!out.contains("onload"));
    assert!(!out.contains("<script"));
    assert!(!out.contains("other.test"));
    assert!(out.contains("#shape"));
}

#[test]
fn captured_assets_and_inline_css_use_local_paths() {
    let base = Url::parse("https://example.test/docs/").unwrap();
    let mut map = ResourceMap::new();
    map.insert(
        UrlKey::of(&base.join("photo.png").unwrap()),
        "saved.png".into(),
    );
    let out = String::from_utf8(rewrite_document(br#"<img src="photo.png"><style>.x {background:url(photo.png)}</style><div style="background:url(photo.png)"></div>"#, "text/html", &base, &map).unwrap()).unwrap();
    assert!(!out.contains("url(photo.png)"));
    assert!(out.contains("saved.png"));
}

#[test]
fn symlink_escape_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let link = dir.path().join("escape");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(status.status.success());
    }
    assert!(
        sdl_grabber::paths::checked_path(dir.path(), std::path::Path::new("escape/body")).is_err()
    );
    assert!(!outside.path().join("body").exists());
}
