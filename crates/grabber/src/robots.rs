//! RFC 9309 group selection and longest matching path rule; Allow wins ties.
#[derive(Clone, Debug, Default)]
pub struct Robots {
    rules: Vec<Rule>,
}
type Rule = (bool, String);
type Group = (Vec<String>, Vec<Rule>);
impl Robots {
    pub fn blocked() -> Self {
        Self {
            rules: vec![(false, "/".into())],
        }
    }
    pub fn parse(text: &str) -> Self {
        let mut groups: Vec<Group> = Vec::new();
        let mut agents = Vec::new();
        let mut rules = Vec::new();
        let mut had_rule = false;
        for line in text.lines().chain(std::iter::once("User-agent: __end__")) {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            if key.trim().eq_ignore_ascii_case("user-agent") {
                if had_rule {
                    groups.push((std::mem::take(&mut agents), std::mem::take(&mut rules)));
                    had_rule = false;
                }
                agents.push(value.to_ascii_lowercase());
            } else if !agents.is_empty()
                && (key.trim().eq_ignore_ascii_case("allow")
                    || key.trim().eq_ignore_ascii_case("disallow"))
            {
                had_rule = true;
                if !value.is_empty() {
                    rules.push((key.trim().eq_ignore_ascii_case("allow"), normalize(value)));
                }
            }
        }
        let specific = groups
            .iter()
            .any(|(a, _)| a.iter().any(|s| s == "swoopdlgrabber"));
        Self {
            rules: groups
                .into_iter()
                .filter(|(a, _)| {
                    a.iter().any(|s| {
                        if specific {
                            s == "swoopdlgrabber"
                        } else {
                            s == "*"
                        }
                    })
                })
                .flat_map(|(_, r)| r)
                .collect(),
        }
    }
    pub fn allows(&self, path: &str) -> bool {
        let path = normalize(path);
        self.rules
            .iter()
            .filter(|(_, pattern)| matches(pattern, &path))
            .max_by_key(|(allow, pattern)| {
                (
                    pattern
                        .bytes()
                        .filter(|b| !matches!(b, b'*' | b'$'))
                        .count(),
                    *allow,
                )
            })
            .is_none_or(|(allow, _)| *allow)
    }
}
fn normalize(value: &str) -> String {
    let mut out = String::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            let digit = |b: u8| {
                if b.is_ascii_digit() {
                    b - b'0'
                } else {
                    b.to_ascii_lowercase() - b'a' + 10
                }
            };
            let n = digit(bytes[i + 1]) * 16 + digit(bytes[i + 2]);
            if n.is_ascii_alphanumeric() || b"-._~".contains(&n) {
                out.push(n as char);
            } else {
                out.push_str(&format!("%{n:02X}"));
            }
            i += 3;
        } else {
            let b = bytes[i];
            if b.is_ascii() {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
            i += 1;
        }
    }
    out
}
fn matches(pattern: &str, path: &str) -> bool {
    let anchored = pattern.ends_with('$');
    let pattern = pattern.strip_suffix('$').unwrap_or(pattern).as_bytes();
    let path = path.as_bytes();
    let (mut p, mut s, mut star, mut retry) = (0, 0, None, 0);
    while s < path.len() {
        if p == pattern.len() && !anchored {
            return true;
        }
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = s;
        } else if p < pattern.len() && pattern[p] == path[s] {
            p += 1;
            s += 1;
        } else if let Some(at) = star {
            retry += 1;
            s = retry;
            p = at + 1;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}
