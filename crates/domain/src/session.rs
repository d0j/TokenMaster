#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct SessionSummary {
    pub id: i64,
    pub started_at_ms: i64,
    pub total_tokens: u64,
    pub model_key: String,
}
