use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("erro de banco de dados: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Fetch(String),
    #[error("erro no motor de torrent: {0}")]
    Torrent(#[from] anyhow::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
