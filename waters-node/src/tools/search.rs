use anyhow::Result;
use serde_json::Value;
use tracing::info;

use super::{Tool, ToolContext};

fn urlencoding(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect()
}

fn extract_href(line: &str) -> Option<String> {
    let start = line.find("href=\"")?;
    let rest = &line[start + 6..];
    let end = rest.find('"')?;
    let href = &rest[..end];
    if href.starts_with("http") { Some(href.to_string()) } else { None }
}

fn extract_title(line: &str) -> Option<String> {
    let start = line.find(">")?;
    let rest = &line[start + 1..];
    let end = rest.find('<')?;
    let title = rest[..end].trim();
    if title.is_empty() { None } else { Some(title.to_string()) }
}

fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c { '<' => in_tag = true, '>' => in_tag = false, _ if !in_tag => result.push(c), _ => {} }
    }
    let mut out = String::new();
    let mut prev_space = false;
    for c in result.chars() {
        if c.is_whitespace() { if !prev_space { out.push(' '); } prev_space = true; }
        else { out.push(c); prev_space = false; }
    }
    out.trim().chars().take(5000).collect()
}

pub fn grep_files() -> Tool {
    Tool {
        name: "grep_files",
        description: "Regex search file contents within workspace. Use `pattern` and optional `include` glob. Skips binary files >1MB.",
        handler: |ctx, args| {
            let pattern = args["pattern"].as_str().ok_or_else(|| anyhow::anyhow!("pattern required"))?;
            let include = args.get("include").and_then(|v| v.as_str()).unwrap_or("*");
            let re = regex::Regex::new(pattern)?;
            let mut results = Vec::new();
            let glob_pattern = format!("{}/**/{}", ctx.workspace, include);
            for entry in glob::glob(&glob_pattern).map_err(|e| anyhow::anyhow!("glob: {}", e))? {
                let entry = entry?;
                if entry.is_file() {
                    if entry.metadata().map(|m| m.len()).unwrap_or(0) > 1_000_000 { continue; }
                    if let Ok(content) = std::fs::read_to_string(&entry) {
                        for (i, line) in content.lines().enumerate() {
                            if re.is_match(line) {
                                let rel = entry.strip_prefix(&ctx.workspace).unwrap_or(&entry).to_string_lossy();
                                results.push(serde_json::json!({"file": rel, "line": i + 1, "match": line}));
                            }
                        }
                    }
                }
            }
            Ok(serde_json::json!({"matches": results, "count": results.len()}))
        },
    }
}

pub fn web_search() -> Tool {
    Tool {
        name: "web_search",
        description: "Search the web. Returns URL + title. Use `query` and optional `region` (us/ru/cn). Default: DuckDuckGo.",
        handler: |_ctx, args| {
            let query = args["query"].as_str().ok_or_else(|| anyhow::anyhow!("query required"))?;
            let region = args.get("region").and_then(|v| v.as_str()).unwrap_or("us");
            info!("web_search: '{}' region={}", &query[..query.len().min(60)], region);

            let url = format!("https://duckduckgo.com/html/?q={}", urlencoding(query));
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(15)).build()?;
            let resp = client.get(&url).header("User-Agent", "Mozilla/5.0 (compatible; waters-node)").send()?;
            let html = resp.text()?;

            let mut results = Vec::new();
            for line in html.lines() {
                if line.contains("class=\"result__a\"") {
                    if let Some(href) = extract_href(line) {
                        if let Some(title) = extract_title(line) {
                            results.push(serde_json::json!({"url": href, "title": title, "region": region}));
                            if results.len() >= 8 { break; }
                        }
                    }
                }
            }
            Ok(serde_json::json!({"query": query, "region": region, "results": results, "count": results.len()}))
        },
    }
}

pub fn fetch_url() -> Tool {
    Tool {
        name: "fetch_url",
        description: "Fetch a URL and return its text content. Use `url`.",
        handler: |_ctx, args| {
            let url = args["url"].as_str().ok_or_else(|| anyhow::anyhow!("url required"))?;
            info!("fetch_url: {}", &url[..url.len().min(80)]);
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("Mozilla/5.0 (compatible; waters-node)")
                .build()?;
            let resp = client.get(url).send()?;
            let content = resp.text()?;
            let stripped = strip_html(&content);
            Ok(serde_json::json!({"url": url, "content": stripped, "chars": stripped.len()}))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_strip_html() {
        let result = strip_html("<p>Hello <b>world</b></p>");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello world"), "hello+world");
        assert_eq!(urlencoding("a/b"), "a%2Fb");
    }
}
