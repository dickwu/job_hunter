use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JobMatch {
    pub id: String,
    pub analysis_id: Option<String>,
    pub url: String,
    pub title: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub match_score: f64,
    pub summary: String,
    pub created_at: String,
    pub raw_excerpt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JobMatchInput {
    pub analysis_id: Option<String>,
    pub url: String,
    pub title: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub match_score: f64,
    pub summary: String,
    pub raw_excerpt: Option<String>,
}

impl Db {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let mut path = app
            .path()
            .app_data_dir()
            .map_err(|err| format!("failed to resolve app data dir: {err}"))?;
        std::fs::create_dir_all(&path)
            .map_err(|err| format!("failed to create app data dir: {err}"))?;
        path.push("job_matches.sqlite");
        Self::from_path(path)
    }

    fn from_path(path: PathBuf) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|err| format!("open db: {err}"))?;
        conn.execute_batch(
            r#"
        CREATE TABLE IF NOT EXISTS job_matches (
          id TEXT PRIMARY KEY,
          analysis_id TEXT,
          url TEXT NOT NULL,
          title TEXT,
          company TEXT,
          location TEXT,
          match_score REAL NOT NULL,
          summary TEXT NOT NULL,
          created_at TEXT NOT NULL,
          raw_excerpt TEXT
        );
        "#,
        )
        .map_err(|err| format!("create table: {err}"))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_match(&self, input: JobMatchInput) -> Result<JobMatch, String> {
        let id = Uuid::new_v4().to_string();
        let created_at: DateTime<Utc> = Utc::now();
        let created_at = created_at.to_rfc3339();

        let match_score = input.match_score;
        let conn = self
            .conn
            .lock()
            .map_err(|_| "db lock poisoned".to_string())?;
        conn
      .execute(
        r#"
        INSERT INTO job_matches
          (id, analysis_id, url, title, company, location, match_score, summary, created_at, raw_excerpt)
        VALUES
          (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        params![
          id,
          input.analysis_id,
          input.url,
          input.title,
          input.company,
          input.location,
          match_score,
          input.summary,
          created_at,
          input.raw_excerpt
        ],
      )
      .map_err(|err| format!("insert job match: {err}"))?;

        Ok(JobMatch {
            id,
            analysis_id: input.analysis_id,
            url: input.url,
            title: input.title,
            company: input.company,
            location: input.location,
            match_score,
            summary: input.summary,
            created_at,
            raw_excerpt: input.raw_excerpt,
        })
    }

    pub fn list_matches(&self, limit: usize) -> Result<Vec<JobMatch>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "db lock poisoned".to_string())?;
        let mut stmt = conn
      .prepare(
        r#"
        SELECT id, analysis_id, url, title, company, location, match_score, summary, created_at, raw_excerpt
        FROM job_matches
        ORDER BY datetime(created_at) DESC
        LIMIT ?1
        "#,
      )
      .map_err(|err| format!("prepare query: {err}"))?;
        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok(JobMatch {
                    id: row.get(0)?,
                    analysis_id: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    company: row.get(4)?,
                    location: row.get(5)?,
                    match_score: row.get::<_, f64>(6)?,
                    summary: row.get(7)?,
                    created_at: row.get(8)?,
                    raw_excerpt: row.get(9)?,
                })
            })
            .map_err(|err| format!("query job matches: {err}"))?;

        let mut matches = Vec::new();
        for row in rows {
            matches.push(row.map_err(|err| format!("row parse: {err}"))?);
        }
        Ok(matches)
    }

    pub fn clear(&self) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "db lock poisoned".to_string())?;
        conn.execute("DELETE FROM job_matches", [])
            .map_err(|err| format!("clear job matches: {err}"))?;
        Ok(())
    }
}
