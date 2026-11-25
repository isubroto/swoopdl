use sdl_grabber::{Decision, ProjectConfig, ResourceKind, evaluate, normalize_url};
use url::Url;

#[test]
fn asset_save_filters_do_not_block_page_traversal() {
    let mut config = sdl_grabber::ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/docs/").unwrap(),
        "output".into(),
    );
    config.extensions.insert("png".into());
    config.mime_types.insert("image/png".into());
    assert!(sdl_grabber::policy::save_allowed(
        &config,
        &config.seed,
        sdl_grabber::ResourceKind::Page,
        "text/html"
    ));
    assert!(sdl_grabber::policy::save_allowed(
        &config,
        &config.seed.join("photo.PNG").unwrap(),
        sdl_grabber::ResourceKind::Asset,
        "image/png; charset=utf-8"
    ));
    assert!(!sdl_grabber::policy::save_allowed(
        &config,
        &config.seed.join("style.css").unwrap(),
        sdl_grabber::ResourceKind::Asset,
        "text/css"
    ));
}

fn config() -> ProjectConfig {
    ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/docs/").unwrap(),
        "capture".into(),
    )
}

#[test]
fn normalization_preserves_query_identity() {
    let base = config().seed;
    assert_eq!(
        normalize_url(&base, "page?a=1&b=2#one").unwrap(),
        normalize_url(&base, "page?a=1&b=2#two").unwrap()
    );
    assert_ne!(
        normalize_url(&base, "page?a=1&b=2").unwrap(),
        normalize_url(&base, "page?b=2&a=1").unwrap()
    );
    assert!(normalize_url(&base, "https://user:secret@example.test/").is_err());
    assert!(normalize_url(&base, "javascript:alert(1)").is_err());
}

#[test]
fn scope_separates_pages_and_assets() {
    let mut c = config();
    c.resource_origins.insert("https://cdn.test".into());
    let cdn = Url::parse("https://cdn.test/style.css").unwrap();
    assert_eq!(evaluate(&c, &cdn, ResourceKind::Asset, 0), Decision::Allow);
    assert_ne!(evaluate(&c, &cdn, ResourceKind::Page, 0), Decision::Allow);
    assert_ne!(
        evaluate(
            &c,
            &Url::parse("https://example.test:444/docs/").unwrap(),
            ResourceKind::Page,
            0
        ),
        Decision::Allow
    );
    assert_ne!(
        evaluate(
            &c,
            &Url::parse("https://example.test/docs-other/").unwrap(),
            ResourceKind::Page,
            0
        ),
        Decision::Allow
    );
}

#[test]
fn exclude_wins() {
    let mut c = config();
    c.include_prefixes.push("https://example.test/docs/".into());
    c.exclude_prefixes
        .push("https://example.test/docs/private".into());
    assert_ne!(
        evaluate(
            &c,
            &Url::parse("https://example.test/docs/private/a").unwrap(),
            ResourceKind::Page,
            1
        ),
        Decision::Allow
    );
}

#[test]
fn seed_depth_is_zero() {
    let c = config();
    assert_eq!(
        evaluate(&c, &c.seed, ResourceKind::Page, 0),
        Decision::Allow
    );
    assert_ne!(
        evaluate(&c, &c.seed, ResourceKind::Page, 4),
        Decision::Allow
    );
    assert_eq!(
        evaluate(&c, &c.seed, ResourceKind::Asset, 4),
        Decision::Allow
    );
}
