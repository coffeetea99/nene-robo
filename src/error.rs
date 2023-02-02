#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(
        #[from]
        std::io::Error,
    ),
    #[error("HTTP error: {0}")]
    Http(
        #[from]
        reqwest::Error,
    ),
    #[error("Environment variable error: {0}")]
    Env(
        #[from]
        std::env::VarError,
    ),
    #[error("JSON parsing error: {0}")]
    Json(
        #[from]
        serde_json::Error,
    ),
    #[error("Database error: {0}")]
    Db(
        #[from]
        rusqlite::Error,
    ),
}
