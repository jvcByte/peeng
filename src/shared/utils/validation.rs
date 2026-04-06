use sea_orm::DbErr;

/// Basic email format validation — checks for a single `@` with non-empty local and domain parts.
pub fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if let Some((local, domain)) = email.split_once('@') {
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    } else {
        false
    }
}

/// Detect a unique constraint violation from a SeaORM `DbErr`.
pub fn is_unique_violation(err: &DbErr) -> bool {
    is_unique_violation_str(&err.to_string())
}
/// Detect a unique constraint violation from an error message string.
/// Use this when the error is wrapped (e.g. `TransactionError`) and you only have the string.
/// Prefers the PostgreSQL error code `23505` for precision; falls back to keyword matching
/// for other DB drivers.
pub fn is_unique_violation_str(msg: &str) -> bool {
    // PostgreSQL error code 23505 is the authoritative signal
    if msg.contains("23505") {
        return true;
    }
    // Fallback for SQLite and other drivers
    let lower = msg.to_lowercase();
    lower.contains("unique constraint") || lower.contains("duplicate key")
}
