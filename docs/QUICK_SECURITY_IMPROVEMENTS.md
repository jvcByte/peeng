# Quick Security Improvements You Can Add Now

## 1. Add Rate Limiting (Recommended)

Install: `cargo add actix-governor`

```rust
// In main.rs
use actix_governor::{Governor, GovernorConfigBuilder};

let governor_conf = GovernorConfigBuilder::default()
    .per_second(2)  // 2 requests per second
    .burst_size(5)  // Allow burst of 5
    .finish()
    .unwrap();

HttpServer::new(move || {
    App::new()
        .wrap(Governor::new(&governor_conf))
        // ... rest of config
})
```

## 2. Add Device/IP Tracking to Refresh Tokens

```sql
-- Add to refresh_tokens table
ALTER TABLE refresh_tokens 
    ADD COLUMN user_agent TEXT,
    ADD COLUMN ip_address INET,
    ADD COLUMN last_used_at TIMESTAMPTZ;
```

```rust
// Store on creation, validate on use
// Alert user if device changes
```

## 3. Add Refresh Token Families (Detect Replay Attacks)

```rust
// If a revoked refresh token is presented, 
// revoke ALL tokens for that user
// This detects token theft
```

## 4. Use HttpOnly Cookies for Web Clients

```rust
// Instead of JSON response with refresh_token
use actix_web::cookie::{Cookie, SameSite};

let cookie = Cookie::build("refresh_token", refresh_plain)
    .http_only(true)
    .secure(true)  // HTTPS only
    .same_site(SameSite::Strict)
    .max_age(Duration::days(30))
    .finish();

HttpResponse::Ok()
    .cookie(cookie)
    .json(/* access token only */)
```

## 5. Add Audit Logging

```rust
// Log all auth events
// - Login attempts (success/failure)
// - Token refreshes
// - Logouts
// - Suspicious activity
```

## 6. Add Email Notifications

```rust
// Notify user of:
// - New device login
// - Password change
// - Token refresh from new location
```

## The Most Important Thing

**Client-side security is critical:**

❌ **Never** store refresh tokens in:
- localStorage (vulnerable to XSS)
- sessionStorage (vulnerable to XSS)
- Unencrypted storage

✅ **Do** store refresh tokens in:
- HttpOnly cookies (web)
- iOS Keychain (iOS)
- Android EncryptedSharedPreferences (Android)
- Secure enclave when available

Your server-side implementation is solid. The biggest risk is how clients store the tokens.
