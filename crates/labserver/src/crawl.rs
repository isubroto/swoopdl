//! Small, logged static-site graph shared by crawl/offline/sync acceptance tests.
use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
pub async fn robots() -> &'static str {
    "User-agent: SwoopdlGrabber\nDisallow: /capture/private\n"
}
pub async fn page(Path(name): Path<String>, headers: HeaderMap) -> Response {
    if name == "padded-mime" || name == "uppercase-mime" {
        let mime = if name == "padded-mime" {
            "text/html ; charset=utf-8"
        } else {
            "TEXT/HTML"
        };
        return (
            [(header::CONTENT_TYPE, mime)],
            "<script>window.captureScriptRan=true</script><p>Safe content</p>",
        )
            .into_response();
    }
    if name == "retry-after" {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "3600")],
        )
            .into_response();
    }
    if name == "font.ttf" {
        return (
            [(header::CONTENT_TYPE, "font/ttf")],
            include_bytes!("../../../website/vendor/plus-jakarta-sans/plus-jakarta-sans-latin.ttf")
                .as_slice(),
        )
            .into_response();
    }
    if name == "chunked" {
        let chunks = futures::stream::iter(
            (0..16).map(|_| Ok::<_, std::io::Error>(bytes::Bytes::from(vec![b'z'; 4096]))),
        );
        return axum::body::Body::from_stream(chunks).into_response();
    }
    if name == "gzip" {
        // 65,536 decoded 'z' bytes: wire length is deliberately below the budget.
        let body = hex::decode("1f8b08000000000000ffedc101010000008090dfadef080a0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006af113329700000100").unwrap();
        return ([(header::CONTENT_ENCODING, "gzip")], body).into_response();
    }
    let (mime, body) = match name.as_str() {
        "index.html" => (
            "text/html",
            r#"<!doctype html><html><head><link rel="stylesheet" href="style.css"></head><body><a href="next.html#part">Next</a><a href="private">Private</a><a href="missing">Broken</a><img src="image.svg" srcset="image.svg 1x, other.svg 2x"><script>fetch('/forbidden')</script></body></html>"#,
        ),
        "next.html" => (
            "text/html",
            r#"<h1 id="part">Next</h1><a href="index.html">Back</a>"#,
        ),
        "style.css" => (
            "text/css",
            r#"@import "nested.css"; body {background-image:url(image.svg)}"#,
        ),
        "nested.css" => (
            "text/css",
            "@font-face {font-family:fixture;src:url(font.ttf)} body {font-family:fixture} h1 {color:green}",
        ),
        "image.svg" | "other.svg" => (
            "image/svg+xml",
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="green"/></svg>"#,
        ),
        "private" => ("text/plain", "must never be requested"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|v| v == "\"capture-v1\"")
    {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::ETAG, "\"capture-v1\""),
        ],
        body,
    )
        .into_response()
}
