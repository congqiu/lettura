use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("{0}")]
    Database(String),
}

impl From<sqlx::Error> for ModelError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::Database(db_err) => {
                if let Some(constraint) = db_err.constraint() {
                    let msg = match constraint {
                        "users_email_key" => "email already exists",
                        "users_username_key" => "username already exists",
                        "idx_entries_user_hashed_url" => "URL already saved",
                        _ => "duplicate record",
                    };
                    return ModelError::Conflict(msg.to_string());
                }
                ModelError::Database(e.to_string())
            }
            _ => ModelError::Database(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_error_display_not_found() {
        let err = ModelError::NotFound("x".into());
        assert!(err.to_string().contains("x"));
    }

    #[test]
    fn model_error_display_conflict() {
        let err = ModelError::Conflict("y".into());
        assert!(err.to_string().contains("y"));
    }

    #[test]
    fn model_error_display_database() {
        let err = ModelError::Database("z".into());
        assert!(err.to_string().contains("z"));
    }
}
