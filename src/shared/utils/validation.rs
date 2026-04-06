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
pub fn is_unique_violation_str(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("unique") || lower.contains("duplicate") || lower.contains("23505")
}
