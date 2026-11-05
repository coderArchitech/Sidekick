//! Web pages to plain text, for URL documents (PRD CTX-2). Deliberately simple:
//! drops script/style/nav-like blocks and tags, keeps block breaks, decodes the
//! common entities.

/// Extracts readable text and the page title (if any).
#[must_use]
pub fn to_text(html: &str) -> (Option<String>, String) {
    let title = between(html, "<title", "</title>").map(|t| decode(strip_tags(t).trim()));
    let mut s = html.to_owned();
    for tag in [
        "script", "style", "noscript", "svg", "head", "nav", "footer",
    ] {
        s = remove_blocks(&s, tag);
    }
    let lower = s.to_lowercase();
    let mut out = String::with_capacity(s.len() / 2);
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let end = s[i..].find('>').map_or(s.len(), |e| i + e + 1);
            let tag = &lower[i..end];
            if [
                "<p", "</p", "<br", "<div", "</div", "<li", "<h1", "<h2", "<h3", "<h4", "<tr",
                "</tr", "<section", "<article",
            ]
            .iter()
            .any(|t| tag.starts_with(t))
            {
                out.push('\n');
            } else if tag.starts_with("<td") || tag.starts_with("<th") {
                out.push('\t');
            }
            i = end;
        } else {
            let next = s[i..].find('<').map_or(s.len(), |n| i + n);
            out.push_str(&s[i..next]);
            i = next;
        }
    }
    let text = decode(&out);
    let cleaned: Vec<String> = text
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    (title.filter(|t| !t.is_empty()), cleaned.join("\n"))
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let lower = s.to_lowercase();
    let start = lower.find(open)?;
    let after = start + lower[start..].find('>')? + 1;
    let end = after + lower[after..].find(close)?;
    Some(&s[after..end])
}

fn remove_blocks(s: &str, tag: &str) -> String {
    let lower = s.to_lowercase();
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(rel) = lower[i..].find(&open) {
        let start = i + rel;
        out.push_str(&s[i..start]);
        i = lower[start..]
            .find(&close)
            .map_or(s.len(), |e| start + e + close.len());
    }
    out.push_str(&s[i..]);
    out
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

fn decode(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_title_and_body_text() {
        let html = r#"<html><head><title>API &amp; Limits</title><style>p{color:red}</style></head>
            <body><nav>Home | Docs</nav><h1>Rate limits</h1><p>The <b>Pro</b> tier allows 600&nbsp;requests/min.</p>
            <script>alert("x")</script><table><tr><td>Free</td><td>60</td></tr></table><footer>© Acme</footer></body></html>"#;
        let (title, text) = to_text(html);
        assert_eq!(title.as_deref(), Some("API & Limits"));
        assert_eq!(
            text,
            "Rate limits\nThe Pro tier allows 600 requests/min.\nFree 60"
        );
    }

    #[test]
    fn handles_plain_text_and_unclosed_tags() {
        assert_eq!(to_text("just text").1, "just text");
        assert_eq!(to_text("<p>open <b>bold").1, "open bold");
    }
}
