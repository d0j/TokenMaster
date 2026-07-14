pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS session_probe (
  id INTEGER PRIMARY KEY,
  started_at_ms INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL CHECK(total_tokens >= 0),
  model_key TEXT NOT NULL CHECK(length(model_key) BETWEEN 1 AND 64)
) STRICT;
CREATE INDEX IF NOT EXISTS session_probe_started_desc
  ON session_probe(started_at_ms DESC, id DESC);
"#;
