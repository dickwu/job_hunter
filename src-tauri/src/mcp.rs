use crate::db::{Db, JobMatchInput};
use crate::settings::{load_settings, save_settings, JobSettings};
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::net::TcpListener as StdTcpListener;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const MCP_VERSION: &str = "0.1";

pub fn start(app: AppHandle, db: Db) -> Result<u16, String> {
    let listener = StdTcpListener::bind("127.0.0.1:0").map_err(|err| format!("mcp bind: {err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("mcp nonblocking: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("mcp local addr: {err}"))?
        .port();
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(err) => {
                log::error!("mcp listener: {err}");
                return;
            }
        };
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(value) => value,
                Err(err) => {
                    log::error!("mcp accept failed: {err}");
                    continue;
                }
            };

            let app = app.clone();
            let db = db.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_client(stream, app, db).await {
                    log::error!("mcp client error: {err}");
                }
            });
        }
    });

    Ok(port)
}

async fn handle_client(
    stream: tokio::net::TcpStream,
    app: AppHandle,
    db: Db,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|err| format!("mcp read: {err}"))?;
        if bytes == 0 {
            break;
        }

        let request: Value = match serde_json::from_str(line.trim()) {
            Ok(value) => value,
            Err(err) => {
                let response = json!({
                  "id": null,
                  "error": { "message": format!("invalid json: {err}") }
                });
                writer
                    .write_all(format!("{response}\n").as_bytes())
                    .await
                    .map_err(|err| format!("mcp write: {err}"))?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

        let response = match method {
            "initialize" => json!({
              "id": id,
              "result": {
                "protocolVersion": MCP_VERSION,
                "serverInfo": { "name": "job-hunter-mcp", "version": "0.1.0" }
              }
            }),
            "list_tools" => json!({
              "id": id,
              "result": {
                "tools": tool_definitions()
              }
            }),
            "call_tool" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match handle_tool(name, arguments, &app, &db).await {
                    Ok(result) => json!({ "id": id, "result": result }),
                    Err(err) => json!({ "id": id, "error": { "message": err } }),
                }
            }
            _ => json!({
              "id": id,
              "error": { "message": format!("unknown method: {method}") }
            }),
        };

        writer
            .write_all(format!("{response}\n").as_bytes())
            .await
            .map_err(|err| format!("mcp write: {err}"))?;
    }

    Ok(())
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
          "name": "set_query_params",
          "description": "Update the UI query parameters for the current analysis.",
          "inputSchema": {
            "type": "object",
            "properties": {
              "url": { "type": "string" },
              "analysisId": { "type": "string" }
            }
          }
        }),
        json!({
          "name": "fetch_content",
          "description": "Retrieve HTML content for a given URL.",
          "inputSchema": {
            "type": "object",
            "properties": {
              "url": { "type": "string" },
              "maxLength": { "type": "number" }
            },
            "required": ["url"]
          }
        }),
        json!({
          "name": "reload_page",
          "description": "Reload the current webview.",
          "inputSchema": { "type": "object" }
        }),
        json!({
          "name": "get_settings",
          "description": "Load job-search settings from the Tauri store.",
          "inputSchema": { "type": "object" }
        }),
        json!({
          "name": "set_settings",
          "description": "Persist job-search settings to the Tauri store.",
          "inputSchema": {
            "type": "object",
            "properties": { "settings": { "type": "object" } }
          }
        }),
        json!({
          "name": "save_job_match",
          "description": "Save a job match to SQLite.",
          "inputSchema": {
            "type": "object",
            "properties": {
              "analysis_id": { "type": "string" },
              "url": { "type": "string" },
              "title": { "type": "string" },
              "company": { "type": "string" },
              "location": { "type": "string" },
              "match_score": { "type": "number" },
              "summary": { "type": "string" },
              "raw_excerpt": { "type": "string" }
            }
          }
        }),
        json!({
          "name": "list_job_matches",
          "description": "List recent job matches.",
          "inputSchema": {
            "type": "object",
            "properties": { "limit": { "type": "number" } }
          }
        }),
        json!({
          "name": "clear_job_matches",
          "description": "Clear saved job matches.",
          "inputSchema": { "type": "object" }
        }),
    ]
}

async fn handle_tool(
    name: &str,
    arguments: Value,
    app: &AppHandle,
    db: &Db,
) -> Result<Value, String> {
    match name {
        "set_query_params" => {
            let payload = json!({
              "url": arguments.get("url").cloned().unwrap_or(Value::Null),
              "analysisId": arguments.get("analysisId").cloned().unwrap_or(Value::Null)
            });
            let _ = app.emit("mcp:set-query-params", payload);
            Ok(json!({ "ok": true }))
        }
        "fetch_content" => {
            let url = arguments
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("url is required")?;
            let max_length = arguments
                .get("maxLength")
                .and_then(|v| v.as_u64())
                .unwrap_or(60_000) as usize;

            let client = reqwest::Client::builder()
                .user_agent("JobHunter/1.0")
                .build()
                .map_err(|err| format!("http client: {err}"))?;
            let response = client
                .get(url)
                .send()
                .await
                .map_err(|err| format!("http fetch: {err}"))?;
            let status = response.status().as_u16();
            let html = response
                .text()
                .await
                .map_err(|err| format!("http body: {err}"))?;
            let trimmed = if html.len() > max_length {
                html[..max_length].to_string()
            } else {
                html.clone()
            };

            let document = Html::parse_document(&trimmed);
            let title_selector = Selector::parse("title").map_err(|err| err.to_string())?;
            let title = document
                .select(&title_selector)
                .next()
                .map(|node| node.text().collect::<String>())
                .unwrap_or_default();
            let text_raw = document.root_element().text().collect::<Vec<_>>().join(" ");
            let whitespace = Regex::new(r"\s+").map_err(|err| err.to_string())?;
            let text = whitespace.replace_all(&text_raw, " ").trim().to_string();
            let text_excerpt = if text.len() > 2000 {
                text[..2000].to_string()
            } else {
                text.clone()
            };

            Ok(json!({
              "status": status,
              "url": url,
              "title": title,
              "html": trimmed,
              "text": text_excerpt
            }))
        }
        "reload_page" => {
            let _ = app.emit("mcp:reload", json!({}));
            Ok(json!({ "ok": true }))
        }
        "get_settings" => {
            let settings = load_settings(app)?.unwrap_or_default();
            Ok(json!({ "settings": settings }))
        }
        "set_settings" => {
            let settings_value = arguments
                .get("settings")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let settings: JobSettings = serde_json::from_value(settings_value)
                .map_err(|err| format!("settings parse: {err}"))?;
            let saved = save_settings(app, &settings)?;
            Ok(json!({ "settings": saved }))
        }
        "save_job_match" => {
            let input: JobMatchInput = serde_json::from_value(arguments)
                .map_err(|err| format!("job match parse: {err}"))?;
            let saved = db.insert_match(input)?;
            let _ = app.emit("analysis:completed", json!({ "match": saved }));
            Ok(json!({ "match": saved }))
        }
        "list_job_matches" => {
            let limit = arguments
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;
            let matches = db.list_matches(limit)?;
            Ok(json!({ "matches": matches }))
        }
        "clear_job_matches" => {
            db.clear()?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}
