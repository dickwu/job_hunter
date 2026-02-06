use crate::settings::JobSettings;
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

pub fn run() {
    if let Err(err) = run_inner() {
        eprintln!("analysis agent failed: {err}");
    }
}

fn run_inner() -> Result<(), String> {
    let port: u16 = std::env::var("JOB_HUNTER_MCP_PORT")
        .map_err(|_| "missing JOB_HUNTER_MCP_PORT".to_string())?
        .parse()
        .map_err(|err| format!("invalid port: {err}"))?;
    let url = std::env::var("JOB_HUNTER_TARGET_URL")
        .map_err(|_| "missing JOB_HUNTER_TARGET_URL".to_string())?;
    let analysis_id = std::env::var("JOB_HUNTER_ANALYSIS_ID").ok();

    let mut client = McpClient::connect(port)?;
    let _ = client.send("initialize", json!({}))?;

    let settings_value = client.send(
        "call_tool",
        json!({ "name": "get_settings", "arguments": {} }),
    )?;
    let settings = settings_value
        .get("settings")
        .cloned()
        .and_then(|value| serde_json::from_value::<JobSettings>(value).ok())
        .unwrap_or_default();

    let content_value = client.send(
        "call_tool",
        json!({ "name": "fetch_content", "arguments": { "url": url, "maxLength": 120000 } }),
    )?;

    let html = content_value
        .get("html")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let text = content_value
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let default_title = content_value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let extracted = extract_listing(&html, &text, &default_title);
    let scored = match_listing(&extracted, &settings);
    let analysis = AnalysisResult {
        url: url.clone(),
        title: extracted.title.clone(),
        company: extracted.company.clone(),
        location: extracted.location.clone(),
        summary: scored.summary,
        match_score: scored.match_score,
        raw_excerpt: extracted.raw_excerpt.clone(),
    };
    let AnalysisResult {
        url,
        title,
        company,
        location,
        summary,
        match_score,
        raw_excerpt,
    } = analysis;

    let url_for_query = url.clone();
    let analysis_id_for_query = analysis_id.clone();

    let _ = client.send(
        "call_tool",
        json!({
          "name": "save_job_match",
          "arguments": {
            "analysis_id": analysis_id,
            "url": url,
            "title": title,
            "company": company,
            "location": location,
            "match_score": match_score,
            "summary": summary,
            "raw_excerpt": raw_excerpt
          }
        }),
    )?;

    let _ = client.send(
        "call_tool",
        json!({
          "name": "set_query_params",
          "arguments": {
            "url": url_for_query,
            "analysisId": analysis_id_for_query
          }
        }),
    )?;
    let _ = client.send(
        "call_tool",
        json!({ "name": "reload_page", "arguments": {} }),
    )?;

    Ok(())
}

struct McpClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: u64,
}

impl McpClient {
    fn connect(port: u16) -> Result<Self, String> {
        let stream =
            TcpStream::connect(("127.0.0.1", port)).map_err(|err| format!("connect mcp: {err}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .map_err(|err| format!("timeout: {err}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|err| format!("timeout: {err}"))?;
        let reader = BufReader::new(stream.try_clone().map_err(|err| err.to_string())?);
        Ok(Self {
            reader,
            writer: stream,
            next_id: 1,
        })
    }

    fn send(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({ "id": id.to_string(), "method": method, "params": params });
        self.writer
            .write_all(format!("{request}\n").as_bytes())
            .map_err(|err| format!("mcp write: {err}"))?;
        self.writer
            .flush()
            .map_err(|err| format!("mcp flush: {err}"))?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|err| format!("mcp read: {err}"))?;
        let response: Value =
            serde_json::from_str(line.trim()).map_err(|err| format!("mcp parse: {err}"))?;
        if let Some(error) = response.get("error") {
            return Err(error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown mcp error")
                .to_string());
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }
}

struct AnalysisResult {
    url: String,
    title: Option<String>,
    company: Option<String>,
    location: Option<String>,
    summary: String,
    match_score: f64,
    raw_excerpt: Option<String>,
}

struct ExtractedListing {
    title: Option<String>,
    company: Option<String>,
    location: Option<String>,
    text: String,
    raw_excerpt: Option<String>,
}

struct MatchResult {
    summary: String,
    match_score: f64,
}

fn extract_listing(html: &str, text: &str, default_title: &str) -> ExtractedListing {
    let document = Html::parse_document(html);
    let h1_selector = Selector::parse("h1").unwrap();
    let title = document
        .select(&h1_selector)
        .next()
        .map(|node| node.text().collect::<String>())
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            if default_title.trim().is_empty() {
                None
            } else {
                Some(default_title.to_string())
            }
        });

    let company = extract_company(&document).or_else(|| {
        title
            .as_ref()
            .and_then(|value| split_company_from_title(value))
    });

    let location = extract_location(text);
    let excerpt = if text.len() > 400 {
        Some(text[..400].to_string())
    } else if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    };

    ExtractedListing {
        title,
        company,
        location,
        text: text.to_string(),
        raw_excerpt: excerpt,
    }
}

fn match_listing(extracted: &ExtractedListing, settings: &JobSettings) -> MatchResult {
    let text_lower = extracted.text.to_lowercase();
    let mut hits = 0.0;
    for keyword in &settings.keywords {
        if text_lower.contains(&keyword.to_lowercase()) {
            hits += 1.0;
        }
    }
    let mut score = if settings.keywords.is_empty() {
        50.0
    } else {
        (hits / settings.keywords.len() as f64) * 100.0
    };

    if let Some(title_value) = &extracted.title {
        let title_lower = title_value.to_lowercase();
        if settings
            .preferred_titles
            .iter()
            .any(|value| !value.is_empty() && title_lower.contains(&value.to_lowercase()))
        {
            score += 10.0;
        }
    }

    if let Some(location_value) = &extracted.location {
        let location_lower = location_value.to_lowercase();
        if settings
            .locations
            .iter()
            .any(|value| !value.is_empty() && location_lower.contains(&value.to_lowercase()))
        {
            score += 6.0;
        }
    }

    if settings.remote_only && text_lower.contains("remote") {
        score += 8.0;
    }
    if let Some(company_name) = &extracted.company {
        if settings
            .company_blacklist
            .iter()
            .any(|c| !c.is_empty() && company_name.to_lowercase().contains(&c.to_lowercase()))
        {
            score -= 15.0;
        }
    }
    score = score.clamp(0.0, 100.0);

    let summary = format!(
        "Matched {:.0}% of keywords. Remote preference: {}. Title signal: {}.",
        score,
        if settings.remote_only { "on" } else { "off" },
        extracted
            .title
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    );

    MatchResult {
        summary,
        match_score: score,
    }
}

fn extract_company(document: &Html) -> Option<String> {
    let meta_selector = Selector::parse("meta").ok()?;
    for node in document.select(&meta_selector) {
        let attrs = node.value();
        let key = attrs
            .attr("property")
            .or_else(|| attrs.attr("name"))
            .unwrap_or("");
        if matches!(key, "og:site_name" | "application-name" | "company") {
            if let Some(content) = attrs.attr("content") {
                return Some(content.to_string());
            }
        }
    }
    None
}

fn split_company_from_title(value: &str) -> Option<String> {
    let separators = [" - ", " | ", " @ "];
    for sep in separators {
        let parts: Vec<&str> = value.split(sep).collect();
        if parts.len() >= 2 {
            return Some(parts.last()?.trim().to_string());
        }
    }
    None
}

fn extract_location(text: &str) -> Option<String> {
    let regex = Regex::new(r"Location[:\s]+([A-Za-z0-9 ,./-]{3,60})").ok()?;
    let caps = regex.captures(text)?;
    let location = caps.get(1)?.as_str().trim();
    if location.is_empty() {
        None
    } else {
        Some(location.to_string())
    }
}
