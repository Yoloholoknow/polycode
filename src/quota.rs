use rusqlite::{params, Connection, Result as SqlResult};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64
}

// ── Schema ────────────────────────────────────────────────────────────────────

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS invocations (
    id           INTEGER PRIMARY KEY,
    adapter_id   TEXT    NOT NULL,
    model        TEXT,
    ts           INTEGER NOT NULL,
    success      INTEGER NOT NULL,
    error_kind   TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER
);

CREATE TABLE IF NOT EXISTS availability (
    adapter_id     TEXT    PRIMARY KEY,
    cooldown_until INTEGER NOT NULL DEFAULT 0,
    last_error     TEXT,
    updated_ts     INTEGER NOT NULL
);
";

// ── Public types ──────────────────────────────────────────────────────────────

pub struct InvocationRecord {
    pub adapter_id: String,
    pub model: Option<String>,
    pub success: bool,
    pub error_kind: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug)]
pub struct StatusRow {
    pub adapter_id: String,
    /// Unix secs; 0 means not cooling down
    pub cooldown_until: i64,
    pub last_error: Option<String>,
    pub invocation_count: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

impl StatusRow {
    pub fn is_cooling_down(&self) -> bool {
        self.cooldown_until > now_secs()
    }
}

// ── QuotaTracker ──────────────────────────────────────────────────────────────

pub struct QuotaTracker {
    conn: Connection,
}

impl QuotaTracker {
    /// Open (or create) the on-disk database at `~/.polycode/quota.db`.
    pub fn open() -> anyhow::Result<Self> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// In-memory database for tests.
    pub fn in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Record an invocation outcome.
    pub fn record_invocation(&self, rec: &InvocationRecord) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO invocations
                (adapter_id, model, ts, success, error_kind, input_tokens, output_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rec.adapter_id,
                rec.model,
                now_secs(),
                rec.success as i32,
                rec.error_kind,
                rec.input_tokens.map(|v| v as i64),
                rec.output_tokens.map(|v| v as i64),
            ],
        )?;
        Ok(())
    }

    /// Mark an adapter as cooling down for the given duration.
    pub fn mark_quota_exceeded(&self, adapter_id: &str, cooldown: Duration) -> SqlResult<()> {
        let until = now_secs() + cooldown.as_secs() as i64;
        self.conn.execute(
            "INSERT INTO availability (adapter_id, cooldown_until, last_error, updated_ts)
             VALUES (?1, ?2, 'QuotaExceeded', ?3)
             ON CONFLICT(adapter_id) DO UPDATE SET
                 cooldown_until = excluded.cooldown_until,
                 last_error     = excluded.last_error,
                 updated_ts     = excluded.updated_ts",
            params![adapter_id, until, now_secs()],
        )?;
        Ok(())
    }

    /// Returns Some(until_secs) if the adapter is still cooling down, None otherwise.
    pub fn is_cooling_down(&self, adapter_id: &str) -> SqlResult<Option<i64>> {
        let now = now_secs();
        let result = self.conn.query_row(
            "SELECT cooldown_until FROM availability WHERE adapter_id = ?1",
            params![adapter_id],
            |row| row.get::<_, i64>(0),
        );
        match result {
            Ok(until) if until > now => Ok(Some(until)),
            Ok(_) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Clear a cooldown on successful invocation.
    pub fn clear_cooldown(&self, adapter_id: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE availability SET cooldown_until = 0, last_error = NULL, updated_ts = ?1
             WHERE adapter_id = ?2",
            params![now_secs(), adapter_id],
        )?;
        Ok(())
    }

    /// Return a summary row per adapter that has any recorded activity.
    pub fn status_rows(&self) -> SqlResult<Vec<StatusRow>> {
        // Collect all known adapter IDs from both tables.
        let mut stmt = self.conn.prepare(
            "SELECT adapter_id FROM (
                 SELECT DISTINCT adapter_id FROM invocations
                 UNION
                 SELECT adapter_id FROM availability
             ) ORDER BY adapter_id",
        )?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<SqlResult<_>>()?;

        let mut rows = Vec::new();
        for id in ids {
            let (count, total_in, total_out): (i64, i64, i64) = self
                .conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0)
                     FROM invocations WHERE adapter_id = ?1",
                    params![id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap_or((0, 0, 0));

            let (cooldown_until, last_error) = self
                .conn
                .query_row(
                    "SELECT cooldown_until, last_error FROM availability WHERE adapter_id = ?1",
                    params![id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .unwrap_or((0, None));

            rows.push(StatusRow {
                adapter_id: id,
                cooldown_until,
                last_error,
                invocation_count: count.max(0) as u64,
                total_input_tokens: total_in.max(0) as u64,
                total_output_tokens: total_out.max(0) as u64,
            });
        }
        Ok(rows)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn db_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".polycode").join("quota.db"))
}

fn init_schema(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> QuotaTracker {
        QuotaTracker::in_memory().unwrap()
    }

    fn rec(adapter_id: &str, success: bool) -> InvocationRecord {
        InvocationRecord {
            adapter_id: adapter_id.to_string(),
            model: Some("test-model".to_string()),
            success,
            error_kind: if success { None } else { Some("QuotaExceeded".to_string()) },
            input_tokens: Some(10),
            output_tokens: Some(5),
        }
    }

    #[test]
    fn record_and_status() {
        let t = tracker();
        t.record_invocation(&rec("claude-code", true)).unwrap();
        t.record_invocation(&rec("claude-code", true)).unwrap();
        let rows = t.status_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].adapter_id, "claude-code");
        assert_eq!(rows[0].invocation_count, 2);
        assert_eq!(rows[0].total_input_tokens, 20);
        assert_eq!(rows[0].total_output_tokens, 10);
    }

    #[test]
    fn mark_quota_exceeded_then_cooling_down() {
        let t = tracker();
        t.mark_quota_exceeded("codex", Duration::from_secs(3600)).unwrap();
        let until = t.is_cooling_down("codex").unwrap();
        assert!(until.is_some(), "should be cooling down");
    }

    #[test]
    fn expired_cooldown_returns_none() {
        let t = tracker();
        // Set cooldown to 1 second ago (already expired)
        t.conn
            .execute(
                "INSERT INTO availability (adapter_id, cooldown_until, updated_ts) VALUES ('codex', ?1, ?2)",
                params![now_secs() - 1, now_secs()],
            )
            .unwrap();
        let result = t.is_cooling_down("codex").unwrap();
        assert!(result.is_none(), "expired cooldown should return None");
    }

    #[test]
    fn clear_cooldown_stops_cooling_down() {
        let t = tracker();
        t.mark_quota_exceeded("aider", Duration::from_secs(3600)).unwrap();
        assert!(t.is_cooling_down("aider").unwrap().is_some());
        t.clear_cooldown("aider").unwrap();
        assert!(t.is_cooling_down("aider").unwrap().is_none());
    }

    #[test]
    fn unknown_adapter_not_cooling_down() {
        let t = tracker();
        assert!(t.is_cooling_down("no-such-adapter").unwrap().is_none());
    }

    #[test]
    fn status_rows_shows_availability() {
        let t = tracker();
        t.record_invocation(&rec("copilot", false)).unwrap();
        t.mark_quota_exceeded("copilot", Duration::from_secs(3600)).unwrap();
        let rows = t.status_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_cooling_down());
        assert_eq!(rows[0].last_error.as_deref(), Some("QuotaExceeded"));
    }
}
