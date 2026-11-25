use sdl_grabber::{ResourceKind, discover::discover};
use url::Url;

#[test]
fn nested_css_and_srcset_are_discovered() {
    let base = Url::parse("https://example.test/docs/index.html").unwrap();
    let html = br#"<base href="/manual/"><a href="next#part">Next</a><link rel="stylesheet" href="style.css"><img srcset="one.png 1x, two.png 2x"><style>@import "extra.css"; .x{background:url(bg.png)}</style><p style="background:url(inline.png)"></p>"#;
    let links = discover(html, "text/html", &base).unwrap();
    for name in [
        "next",
        "style.css",
        "one.png",
        "two.png",
        "extra.css",
        "bg.png",
        "inline.png",
    ] {
        assert!(
            links
                .iter()
                .any(|l| l.url.as_str() == format!("https://example.test/manual/{name}")),
            "{name}"
        );
    }
    assert_eq!(
        links
            .iter()
            .find(|l| l.url.path().ends_with("next"))
            .unwrap()
            .kind,
        ResourceKind::Page
    );
    assert_eq!(
        links
            .iter()
            .find(|l| l.url.path().ends_with("next"))
            .unwrap()
            .fragment
            .as_deref(),
        Some("part")
    );
}

#[test]
fn css_tokenization_handles_escapes_and_nested_functions() {
    let links = discover(
        br#"@import "theme.css"; .x { background: image-set(url("a\2e png") 1x, url(b.png) 2x); }"#,
        "text/css",
        &Url::parse("https://example.test/").unwrap(),
    )
    .unwrap();
    assert_eq!(
        links.iter().map(|l| l.url.path()).collect::<Vec<_>>(),
        ["/theme.css", "/a.png", "/b.png"]
    );
}

#[test]
fn binary_resources_do_not_consume_the_document_parse_cap() {
    let body = vec![0u8; (8 << 20) + 1];
    assert!(
        discover(
            &body,
            "application/octet-stream",
            &Url::parse("https://example.test/file.bin").unwrap()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn discovery_cannot_exceed_the_100000_url_ceiling() {
    let body: String = (0..100_002)
        .map(|i| format!("<a href='?n={i}'>x</a>"))
        .collect();
    assert!(
        discover(
            body.as_bytes(),
            "text/html",
            &Url::parse("https://example.test/").unwrap()
        )
        .is_err()
    );
}
