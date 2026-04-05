# peeng

Backend API for **peeng**, a delivery app. Built with [Actix-Web](https://actix.rs/) and [SeaORM](https://www.sea-ql.org/SeaORM/), providing user management and JWT-based authentication with rotating refresh tokens backed by PostgreSQL.

## Features

- User registration and login with Argon2id password hashing
- Short-lived JWT access tokens (HS256) with token versioning for immediate revocation
- Opaque rotating refresh tokens stored in the database
- Per-session and global logout
- Protected routes via an Actix extractor (`AuthenticatedUser`)
- Auto-runs database migrations on startup
- CORS configured for local frontend dev (localhost:3000, localhost:1420, Tauri)

## Tech Stack

| Layer | Library |
|---|---|
| Web framework | actix-web 4 |
| ORM | sea-orm 1 |
| Database | PostgreSQL |
| Password hashing | argon2 |
| JWT | jsonwebtoken 10 |
| Migrations | sea-orm-migration |

## Project Structure

```
src/
  main.rs                  # Server bootstrap, CORS, migrations
  api/
    routes.rs              # Mounts all routes under /api
    auth/                  # Register, login, refresh, logout handlers
    users/                 # CRUD handlers (protected)
    home/                  # Health check, DB ping
  shared/
    config/                # AppState, env loading, DB init
    middleware/auth.rs     # AuthenticatedUser extractor
    errors/api_errors.rs   # Typed error enum → HTTP responses
    utils/auth_utils.rs    # JWT helpers, Argon2 helpers, token generation
entity/                    # SeaORM entity definitions (users, refresh_tokens)
migration/                 # SeaORM migrations
```

## Getting Started

### Prerequisites

- Rust (edition 2024)
- PostgreSQL

### Setup

1. Copy the example env file and fill in your values:

```bash
cp .env.example .env
```

```env
DATABASE_URL=postgresql://username:password@localhost:5432/your_database
JWT_SECRET=your_super_secret_jwt_key_here
JWT_ACCESS_TOKEN_EXPIRATION_MINUTES=15
JWT_REFRESH_TOKEN_EXPIRATION_DAYS=30
ADDRESS=127.0.0.1
PORT=8080
RUST_LOG=debug
```

2. Run the server (migrations run automatically on startup):

```bash
cargo run
```

## API Reference

All endpoints are prefixed with `/api`. A full list is available at `GET /api`.

### Health

| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/api/health` | No | Health check |
| GET | `/api/db_conn` | No | Database connectivity check |

### Auth

| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/api/auth/register` | No | Register a new user |
| POST | `/api/auth/login` | No | Login and receive tokens |
| POST | `/api/auth/refresh` | No | Rotate refresh token, get new access token |
| POST | `/api/auth/logout` | No | Revoke a refresh token and invalidate access tokens |
| POST | `/api/auth/logout-all` | Bearer | Revoke all sessions for the current user |
| GET | `/api/auth/me` | Bearer | Get current user info |
| POST | `/api/auth/cleanup-tokens` | No | Delete expired refresh tokens |

### Users

All user endpoints require a valid `Authorization: Bearer <token>` header.

| Method | Path | Description |
|---|---|---|
| GET | `/api/users` | List all users |
| GET | `/api/users/{id}` | Get a user by UUID |
| PUT | `/api/users/{id}` | Update a user |
| DELETE | `/api/users/{id}` | Delete a user |

### Example Payloads

**Register / Login**
```json
{ "name": "Alice", "email": "[email]", "password": "s3cureP@ss" }
{ "email": "[email]", "password": "s3cureP@ss" }
```

**Token response**
```json
{
  "access_token": "<jwt>",
  "token_type": "Bearer",
  "expires_in": 900,
  "refresh_token": "<opaque>",
  "user": { "id": "<uuid>", "name": "Alice", "email": "[email]" }
}
```

**Refresh / Logout**
```json
{ "refresh_token": "<opaque>" }
```

## Authentication Flow

1. Register or login → receive an access token (JWT, short-lived) and a refresh token (opaque, long-lived).
2. Include the access token in the `Authorization: Bearer` header for protected routes.
3. When the access token expires, call `/api/auth/refresh` with the refresh token to get a new pair (token rotation).
4. On logout, call `/api/auth/logout` with the refresh token. This revokes the token server-side and increments the user's `token_version`, immediately invalidating any outstanding access tokens.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
