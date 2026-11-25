use sdl_grabber::robots::Robots;

#[test]
fn robots_longest_match_allow_tie_and_agent_groups() {
    let r = Robots::parse(
        "User-agent: *\nDisallow: /all\n\nUser-agent: SwoopdlGrabber\nDisallow: /private\nAllow: /private/open\nDisallow: /private/open\nDisallow: /*.pdf$\n",
    );
    assert!(!r.allows("/private/secret"));
    assert!(r.allows("/private/open"));
    assert!(r.allows("/all"));
    assert!(!r.allows("/file.pdf"));
    assert!(r.allows("/file.pdf?view=1"));
}

#[test]
fn malformed_percent_before_unicode_does_not_panic() {
    let r = Robots::parse("User-agent: *\nDisallow: /%€\n");
    assert!(!r.allows("/%€"));
}
