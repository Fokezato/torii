use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{}", db_message(.0))]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Fetch(String),
    #[error("{}", torrent_message(.0))]
    Torrent(#[from] anyhow::Error),
}

fn db_message(e: &sqlx::Error) -> String {
    tr!("erro de banco de dados: {e}", "database error: {e}")
}

fn torrent_message(e: &anyhow::Error) -> String {
    tr!("erro no motor de torrent: {e}", "torrent engine error: {e}")
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
