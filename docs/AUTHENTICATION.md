# Authentication Guide

This document explains how the authentication system in this project works, shows example HTTP requests (curl) for the supported flows, and gives platform-specific guidance for mobile clients (iOS / Android). The implementation uses short-lived JWT access tokens and rotateable opaque refresh tokens (server-side hashed storage).

Security summary
- Access token: short-lived JWT (HS256 by default). Use it in requests with `Authorization: Bearer <access_token>`.
- Refresh token: opaque random token (rotated on use). The server stores only a secure hash of the refresh token.
- Passwords: hashed using Argon2.
- Use HTTPS for all requests in production. Keep secrets (JWT secret) in a secure secret manager.

Environment variables (example)
- `DATABASE_URL` — PostgreSQL connection string (required).
- `JWT_SECRET` — HMAC secret for signing JWTs (required).
- `JWT_EXP_MINUTES` — Access token lifetime in minutes (default: 15).
- `REFRESH_TOKEN_EXP_DAYS` — Refresh token lifetime in days (default: 30).

Endpoints
- `POST /api/auth/register`
  - Registers a new user, returns an access token and a refresh token.
- `POST /api/auth/login`
  - Authenticates credentials, returns an access token and a refresh token.
- `POST /api/auth/refresh`
  - Accepts a refresh token, validates it, rotates it and returns a new access token + refresh token.
- `POST /api/auth/logout`
  - Accepts a refresh token and revokes it server-side.
- `GET /api/auth/me`
  - Protected endpoint. Requires `Authorization: Bearer <access_token>`.

JSON payloads
- Register:
  - Request:
```crud-with-sea_orm/AUTHENTICATION.md#L301-L314
{
  "name": "Alice",
  "email": "alice@example.com",
  "password": "S3cureP@ssw0rd"
}
```
  - Response (201 Created):
```crud-with-sea_orm/AUTHENTICATION.md#L315-L329
{
  "access_token": "<JWT_ACCESS_TOKEN>",
  "token_type": "Bearer",
  "expires_in": 900,
  "refresh_token": "<OPAQUE_REFRESH_TOKEN>",
  "user": {
    "id": "uuid",
    "name": "Alice",
    "email": "alice@example.com"
  }
}
```

- Login:
  - Request:
```crud-with-sea_orm/AUTHENTICATION.md#L330-L339
{
  "email": "alice@example.com",
  "password": "S3cureP@ssw0rd"
}
```
  - Response (200 OK): same shape as register response (access + refresh + user).

- Refresh:
  - Request:
```crud-with-sea_orm/AUTHENTICATION.md#L340-L343
{
  "refresh_token": "<OPAQUE_REFRESH_TOKEN>"
}
```
  - Response (200 OK):
```crud-with-sea_orm/AUTHENTICATION.md#L344-L354
{
  "access_token": "<NEW_JWT_ACCESS_TOKEN>",
  "token_type": "Bearer",
  "expires_in": 900,
  "refresh_token": "<NEW_OPAQUE_REFRESH_TOKEN>"
}
```

- Logout:
  - Request:
```crud-with-sea_orm/AUTHENTICATION.md#L355-L358
{
  "refresh_token": "<OPAQUE_REFRESH_TOKEN>"
}
```
  - Response: 204 No Content

- Me (protected):
  - Request header:
```
Authorization: Bearer <JWT_ACCESS_TOKEN>
```
  - Response (200 OK):
```crud-with-sea_orm/AUTHENTICATION.md#L359-L365
{
  "id": "uuid",
  "name": "Alice",
  "email": "alice@example.com"
}
```

Curl examples
- Register:
```crud-with-sea_orm/AUTHENTICATION.md#L366-L372
curl -X POST https://api.example.com/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"name":"Alice","email":"alice@example.com","password":"S3cureP@ssw0rd"}'
```

- Login:
```crud-with-sea_orm/AUTHENTICATION.md#L373-L379
curl -X POST https://api.example.com/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"alice@example.com","password":"S3cureP@ssw0rd"}'
```

- Refresh:
```crud-with-sea_orm/AUTHENTICATION.md#L380-L386
curl -X POST https://api.example.com/api/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token":"<OPAQUE_REFRESH_TOKEN>"}'
```

- Logout:
```crud-with-sea_orm/AUTHENTICATION.md#L387-L393
curl -X POST https://api.example.com/api/auth/logout \
  -H "Content-Type: application/json" \
  -d '{"refresh_token":"<OPAQUE_REFRESH_TOKEN>"}'
```

- Get authenticated user:
```crud-with-sea_orm/AUTHENTICATION.md#L394-L399
curl -X GET https://api.example.com/api/auth/me \
  -H "Authorization: Bearer <JWT_ACCESS_TOKEN>"
```

Mobile guidance (recommended best practices)

General
- Always use HTTPS.
- Do not persist access tokens in long-lived storage. Keep them in memory where possible and only for the UI lifecycle.
- Store refresh tokens in a secure, platform-provided secure store:
  - iOS: Keychain (preferably the `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` or better).
  - Android: EncryptedSharedPreferences (using AndroidX Security) or KeyStore-backed storage.
- Treat refresh tokens like passwords — keep them secret, never leak in logs or crash reports.

Typical flow on mobile
1. User registers or logs in. Server returns:
   - short-lived access token (JWT)
   - refresh token (opaque)
2. Mobile app:
   - store refresh token in secure storage (Keychain / EncryptedSharedPreferences).
   - keep access token in memory (used to make requests in Authorization header).
3. On 401 (unauthorized) due to expired access token:
   - call `/api/auth/refresh` with stored refresh token.
   - if refresh succeeds, replace both tokens: update secure storage with the new refresh token and update memory with new access token.
   - if refresh fails (revoked/expired), force re-authentication (show login UI).
4. On logout:
   - Call `/api/auth/logout` with the refresh token, clear secure storage and in-memory access token.

iOS (Swift) pseudocode
```crud-with-sea_orm/AUTHENTICATION.md#L400-L430
// This is pseudocode — use Apple's Keychain APIs or a library like KeychainAccess
func storeRefreshToken(_ token: String) {
  KeychainService.set(key: "refresh_token", value: token)
}

func getRefreshToken() -> String? {
  return KeychainService.get(key: "refresh_token")
}

func callProtectedEndpoint() {
  let accessToken = session.accessToken // kept in memory
  var req = URLRequest(url: URL(string:"https://api.example.com/api/auth/me")!)
  req.addValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
  // execute request
}
```

Android (Kotlin) pseudocode
```crud-with-sea_orm/AUTHENTICATION.md#L431-L459
// Use EncryptedSharedPreferences + MasterKey from AndroidX Security
fun storeRefreshToken(token: String) {
  val prefs = EncryptedSharedPreferences.create(...)
  prefs.edit().putString("refresh_token", token).apply()
}

fun getRefreshToken(): String? {
  val prefs = EncryptedSharedPreferences.create(...)
  return prefs.getString("refresh_token", null)
}

fun callProtectedEndpoint() {
  val access = Session.accessToken // keep in memory
  val request = Request.Builder()
    .url("https://api.example.com/api/auth/me")
    .addHeader("Authorization", "Bearer $access")
    .build()
  // execute request
}
```

Token rotation and security notes
- Rotation: When a refresh token is used, the server issues a new refresh token and revokes the old one (single-use). The client must replace stored token with new value returned by the refresh endpoint.
- Revocation: Server can revoke refresh tokens (e.g., on logout or detected compromise). The client should treat failed refresh as a sign to re-authenticate.
- Token versioning: The implementation includes `token_version` in user records; incrementing it (e.g., on password reset or global logout) will invalidate all existing access tokens.
- Use short access token lifetime (e.g., 10–30 minutes) and refresh token lifetime appropriate for your app (e.g., 30 days for mobile).
- Rate limit login/refresh endpoints at infra or application level to mitigate brute-force attacks.
- Avoid storing refresh tokens in insecure places (local storage, unencrypted SharedPreferences, etc.).

Server-side operational guidance
- Keep `JWT_SECRET` in a secret manager (not in source control).
- Consider rotating the JWT signing key: support `kid` in token headers and expose JWKS / rotation plan if multiple services must validate tokens.
- Monitor refresh token usage patterns to detect anomalies (rapid rotation, repeated failures).
- Periodically remove or archive expired refresh tokens (a background job / DB index).
- Use HTTPS, HSTS, and strict CORS policies for web clients.

Production checklist
- Ensure migrations have been applied to add auth-related columns and refresh token table.
- Set environment variables securely.
- Verify TLS termination and that your API is only accessible via HTTPS.
- Add logging & monitoring around auth endpoints (but never log tokens or passwords).
- Add rate limiting and bot protection on login/refresh endpoints.

Questions or follow-ups
- If you want, I can:
  - produce concrete client-side examples (complete Swift / Kotlin snippets).
  - add cookie-based refresh token variant for web apps (HttpOnly, Secure, SameSite).
  - add unit / integration tests for the auth flows (recommended).
