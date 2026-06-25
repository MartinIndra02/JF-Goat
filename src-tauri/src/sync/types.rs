use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SyncProgress {
    pub current: u32,
    pub total: u32,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncError {
    pub message: String,
    pub batch_start: u32,
    pub batch_size: u32,
    pub retries_attempted: u32,
    pub is_fatal: bool,
}
