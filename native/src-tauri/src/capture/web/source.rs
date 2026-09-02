//! Source detection for web captures.
//!
//! Provenance is derived here, on the Rust side, from the captured URL —
//! never taken from whatever the browser claimed the source was. The
//! extension is trusted to *read* a page; it is not trusted to *label* it.
//!
//! Everything in this module is a pure function over a URL string so it can
//! be tested without a browser, a network, or a vault.

/// The pieces of a URL Relay actually uses. Deliberately not a full RFC 3986
/// parser: capture only ever sees `http`/`https` URLs handed over by a
/// browser that already parsed them, and pulling in a URL crate for host and
/// path would be a dependency for two `split` calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    pub scheme: String,
    /// Lowercased host with any port and userinfo removed.
    pub host: String,
    /// Path with its leading slash, query and fragment removed.
    pub path: String,
}

/// Parses a URL far enough to classify it. Returns `None` for anything that
/// is not an absolute `http`/`https` URL with a host — `file:`, `chrome:`,
/// `javascript:` and friends are rejected here rather than deeper in.
pub fn parse_url(raw: &str) -> Option<ParsedUrl> {
    let trimmed = raw.trim();
    let (scheme, rest) = trimmed.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }

    let rest = rest.split(['#', '?']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    // Strip userinfo (`user:pass@host`) before reading the host, so a
    // `https://github.com@evil.example/` style URL is attributed to
    // `evil.example` and not to GitHub.
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host_port
        .rsplit_once(':')
        .filter(|(h, p)| !h.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        .map_or(host_port, |(h, _)| h)
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();

    if host.is_empty() {
        return None;
    }

    Some(ParsedUrl {
        scheme,
        host,
        path: if path.is_empty() { "/".to_string() } else { path.to_string() },
    })
}

/// What Relay knows about a capture's origin before looking at its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedSource {
    /// Host with a leading `www.` removed — the stable identity of the site.
    pub domain: String,
    /// Human-facing name of the application or site the capture came from.
    pub application: String,
    /// What the URL alone suggests this is, when the URL is enough to tell.
    /// `None` means "ask the extracted content".
    pub capture_type: Option<String>,
}

pub const CAPTURE_TYPE_CONVERSATION: &str = "conversation";
pub const CAPTURE_TYPE_ARTICLE: &str = "article";
pub const CAPTURE_TYPE_REPOSITORY: &str = "repository";
pub const CAPTURE_TYPE_ISSUE: &str = "issue";
pub const CAPTURE_TYPE_PULL_REQUEST: &str = "pull_request";
pub const CAPTURE_TYPE_DISCUSSION: &str = "discussion";
pub const CAPTURE_TYPE_CODE: &str = "code";
pub const CAPTURE_TYPE_PAGE: &str = "page";

/// Strips a leading `www.` so `www.github.com` and `github.com` are one site.
pub fn registrable_domain(host: &str) -> String {
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// Derives domain, application name, and (where the URL is decisive) capture
/// type from a URL.
///
/// A site Relay has never heard of still gets a usable answer: its own domain
/// as the application name. There is no "unknown source" state — every
/// capture can say where it came from.
pub fn detect(url: &str) -> Option<DetectedSource> {
    let parsed = parse_url(url)?;
    let domain = registrable_domain(&parsed.host);

    let (application, capture_type) = match domain.as_str() {
        "chatgpt.com" | "chat.openai.com" => ("ChatGPT", conversation_if(&parsed.path, "/c/")),
        "claude.ai" => ("Claude", conversation_if(&parsed.path, "/chat/")),
        "gemini.google.com" => ("Gemini", conversation_if(&parsed.path, "/app/")),
        "grok.com" | "x.ai" => ("Grok", conversation_if(&parsed.path, "/chat/")),
        "perplexity.ai" => ("Perplexity", conversation_if(&parsed.path, "/search/")),
        "github.com" => ("GitHub", Some(github_capture_type(&parsed.path))),
        "gitlab.com" => ("GitLab", None),
        "stackoverflow.com" => ("Stack Overflow", None),
        "news.ycombinator.com" => ("Hacker News", None),
        "developer.mozilla.org" => ("MDN Web Docs", Some(CAPTURE_TYPE_ARTICLE.to_string())),
        "en.wikipedia.org" => ("Wikipedia", Some(CAPTURE_TYPE_ARTICLE.to_string())),
        "docs.google.com" => ("Google Docs", None),
        "notion.so" | "www.notion.so" => ("Notion", None),
        "linear.app" => ("Linear", None),
        _ => (domain.as_str(), None),
    };

    Some(DetectedSource {
        domain: domain.clone(),
        application: application.to_string(),
        capture_type,
    })
}

/// A chat host's conversation URLs are distinguishable from its landing and
/// settings pages by path prefix; anything else on the same host is left for
/// the content to classify.
fn conversation_if(path: &str, prefix: &str) -> Option<String> {
    if path.starts_with(prefix) {
        Some(CAPTURE_TYPE_CONVERSATION.to_string())
    } else {
        None
    }
}

/// GitHub's URL shape is the classification: `/owner/repo/pull/12` is a pull
/// request whatever the page happens to render.
fn github_capture_type(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        [_owner, _repo, "issues", rest @ ..] if !rest.is_empty() => CAPTURE_TYPE_ISSUE.to_string(),
        [_owner, _repo, "pull", rest @ ..] if !rest.is_empty() => {
            CAPTURE_TYPE_PULL_REQUEST.to_string()
        }
        [_owner, _repo, "discussions", rest @ ..] if !rest.is_empty() => {
            CAPTURE_TYPE_DISCUSSION.to_string()
        }
        [_owner, _repo, "blob", ..] | [_owner, _repo, "tree", ..] => CAPTURE_TYPE_CODE.to_string(),
        [_owner, _repo] => CAPTURE_TYPE_REPOSITORY.to_string(),
        [_owner, _repo, ..] => CAPTURE_TYPE_REPOSITORY.to_string(),
        _ => CAPTURE_TYPE_PAGE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scheme_host_and_path() {
        let parsed = parse_url("https://Example.COM:8443/a/b?q=1#frag").unwrap();
        assert_eq!(parsed.scheme, "https");
        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.path, "/a/b");
    }

    #[test]
    fn defaults_missing_path_to_root() {
        assert_eq!(parse_url("https://example.com").unwrap().path, "/");
    }

    #[test]
    fn rejects_non_http_schemes() {
        for url in [
            "file:///c:/secrets.txt",
            "chrome://settings",
            "javascript:alert(1)",
            "ftp://example.com/x",
            "not a url",
            "",
        ] {
            assert!(parse_url(url).is_none(), "should reject {url}");
        }
    }

    #[test]
    fn attributes_userinfo_urls_to_the_real_host() {
        let parsed = parse_url("https://github.com@evil.example/repo").unwrap();
        assert_eq!(parsed.host, "evil.example");
        assert_eq!(detect("https://github.com@evil.example/repo").unwrap().application, "evil.example");
    }

    #[test]
    fn ipv6_hosts_survive_parsing() {
        assert_eq!(parse_url("http://[::1]:3000/page").unwrap().host, "::1");
    }

    #[test]
    fn detects_chatgpt_conversations_but_not_its_landing_page() {
        let convo = detect("https://chatgpt.com/c/abc-123").unwrap();
        assert_eq!(convo.application, "ChatGPT");
        assert_eq!(convo.domain, "chatgpt.com");
        assert_eq!(convo.capture_type.as_deref(), Some(CAPTURE_TYPE_CONVERSATION));

        let landing = detect("https://chatgpt.com/").unwrap();
        assert_eq!(landing.application, "ChatGPT");
        assert_eq!(landing.capture_type, None);
    }

    #[test]
    fn detects_claude_conversations() {
        let d = detect("https://claude.ai/chat/9f0").unwrap();
        assert_eq!(d.application, "Claude");
        assert_eq!(d.capture_type.as_deref(), Some(CAPTURE_TYPE_CONVERSATION));
    }

    #[test]
    fn classifies_github_by_path_shape() {
        let cases = [
            ("https://github.com/relay/relay", CAPTURE_TYPE_REPOSITORY),
            ("https://github.com/relay/relay/issues/42", CAPTURE_TYPE_ISSUE),
            ("https://github.com/relay/relay/pull/7", CAPTURE_TYPE_PULL_REQUEST),
            ("https://github.com/relay/relay/discussions/3", CAPTURE_TYPE_DISCUSSION),
            ("https://github.com/relay/relay/blob/main/src/lib.rs", CAPTURE_TYPE_CODE),
            ("https://github.com/relay/relay/issues", CAPTURE_TYPE_REPOSITORY),
            ("https://github.com/relay", CAPTURE_TYPE_PAGE),
        ];
        for (url, expected) in cases {
            assert_eq!(
                detect(url).unwrap().capture_type.as_deref(),
                Some(expected),
                "wrong classification for {url}"
            );
        }
    }

    #[test]
    fn unknown_sites_are_named_after_their_domain() {
        let d = detect("https://www.some-blog.example/posts/1").unwrap();
        assert_eq!(d.domain, "some-blog.example");
        assert_eq!(d.application, "some-blog.example");
        assert_eq!(d.capture_type, None);
    }

    #[test]
    fn detect_rejects_unsupported_urls() {
        assert!(detect("chrome://extensions").is_none());
    }
}
