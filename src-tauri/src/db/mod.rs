pub mod media;
pub mod schema;
pub mod servers;

pub const SQLITE_BUSY_TIMEOUT_MS: u64 = 5_000;
pub const SQLITE_WRITE_POOL_SIZE: u32 = 1;

pub fn default_sqlite_read_pool_size() -> u32 {
    std::thread::available_parallelism()
        .map(|parallelism| (parallelism.get() as u32).clamp(4, 16))
        .unwrap_or(4)
}

pub use schema::init_db;
