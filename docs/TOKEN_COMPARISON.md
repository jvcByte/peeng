# Access Token vs Refresh Token

## Side-by-Side Comparison

| Feature | Access Token (JWT) | Refresh Token |
|---------|-------------------|---------------|
| **Type** | JWT (JSON Web Token) | Opaque random string |
| **Lifetime** | 15 minutes | 30 days |
| **Storage** | Client memory (ideally) | Secure storage (Keychain/Cookie) |
| **Used for** | Every API request | Only refresh endpoint |
| **Frequency** | Hundreds of times | Once per 15 minutes |
| **Can revoke?** | ❌ No (stateless) | ✅ Yes (in database) |
| **Database check?** | ❌ No (fast) | ✅ Yes (on refresh only) |
| **If stolen** | 15 min window | Can be revoked |
| **Contains** | User ID, expiry, token version | Random bytes (hashed) |

## Real-World Analogy

### Access Token = Movie Ticket
- Shows you paid (authenticated)
- Valid for specific time (15 min)
- Can't be cancelled once issued
- Guard checks it at door (every request)
- If lost/stolen: Only works until expiry

### Refresh Token = Season Pass
- Lets you get new tickets
- Valid for long time (30 days)
- Can be cancelled/revoked
- Only used at ticket booth (refresh endpoint)
- If lost/stolen: Can be deactivated

## Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ User Login                                                   │
├─────────────────────────────────────────────────────────────┤
│ POST /api/auth/login                                        │
│ { email, password }                                         │
│                                                             │
│ Response:                                                   │
│ {                                                           │
│   access_token: "eyJ..." (expires in 15 min)              │
│   refresh_token: "abc123..." (expires in 30 days)         │
│ }                                                           │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ Making API Requests (next 15 minutes)                       │
├─────────────────────────────────────────────────────────────┤
│ GET /api/auth/me                                            │
│ Authorization: Bearer eyJ... (access token)                 │
│                                                             │
│ GET /api/users                                              │
│ Authorization: Bearer eyJ... (access token)                 │
│                                                             │
│ ... hundreds of requests ...                                │
│ All use the SAME access token                               │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ After 15 Minutes - Access Token Expired                     │
├─────────────────────────────────────────────────────────────┤
│ GET /api/auth/me                                            │
│ Authorization: Bearer eyJ... (expired)                      │
│                                                             │
│ Response: 401 Unauthorized                                  │
│ { error: "Token expired" }                                  │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ Client Refreshes Token                                       │
├─────────────────────────────────────────────────────────────┤
│ POST /api/auth/refresh                                      │
│ { refresh_token: "abc123..." }                              │
│                                                             │
│ Response:                                                   │
│ {                                                           │
│   access_token: "eyJ..." (NEW, expires in 15 min)         │
│   refresh_token: "xyz789..." (NEW, expires in 30 days)    │
│ }                                                           │
│                                                             │
│ Old refresh token is REVOKED                                │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ Continue Making Requests                                     │
├─────────────────────────────────────────────────────────────┤
│ Use NEW access token for next 15 minutes                    │
│ Repeat refresh process when it expires                      │
│ Can continue for 30 days without re-entering password       │
└─────────────────────────────────────────────────────────────┘
```

## Why Not Just One Long-Lived JWT?

### ❌ Bad: 30-Day JWT (No Refresh Token)

```
Problem: If JWT is stolen, attacker has 30 days of access
- Can't revoke it (JWTs are stateless)
- Can't detect theft
- Can't force re-authentication
```

### ✅ Good: 15-Min JWT + 30-Day Refresh Token

```
Benefits:
- If JWT stolen: Only 15 min window
- If refresh token stolen: Can revoke it
- Can force logout (revoke refresh token)
- Can detect suspicious activity (refresh from new device)
- User stays logged in for 30 days (good UX)
```

## Security Layers

```
Layer 1: Short-lived access token (15 min)
└─ Limits damage from theft
└─ Reduces attack window

Layer 2: Long-lived refresh token (30 days)
└─ Can be revoked
└─ Stored securely (hashed in DB)
└─ Rotated on each use

Layer 3: Token version
└─ Invalidates all tokens on logout
└─ Invalidates all tokens on password change

Layer 4: Expiry
└─ Refresh token expires after 30 days
└─ Forces re-authentication
```

## Common Misconceptions

### ❌ "Why not make JWT last 30 days?"
Because you can't revoke JWTs. If stolen, attacker has 30 days.

### ❌ "Why not make refresh token last 15 minutes?"
Then user has to re-enter password every 15 minutes (bad UX).

### ❌ "Why not check JWT against database?"
That defeats the purpose of JWTs (stateless, fast, scalable).

### ✅ "Use both tokens for their strengths"
- JWT: Fast, stateless, short-lived
- Refresh: Revocable, long-lived, secure

## Configuration Recommendations

### High Security (Banking, Healthcare)
```env
JWT_EXP_MINUTES=5          # Very short
REFRESH_TOKEN_EXP_DAYS=7   # Force re-auth weekly
```

### Balanced (Most Apps) ← **Your Current Setup**
```env
JWT_EXP_MINUTES=15         # Good balance
REFRESH_TOKEN_EXP_DAYS=30  # Monthly re-auth
```

### User-Friendly (Low Risk Apps)
```env
JWT_EXP_MINUTES=60         # 1 hour
REFRESH_TOKEN_EXP_DAYS=90  # Quarterly re-auth
```

## Your Current Setup Analysis

```env
JWT_EXP_MINUTES=15
REFRESH_TOKEN_EXP_DAYS=30
```

✅ **This is the industry standard**
- Used by Google, Auth0, Firebase
- Good balance of security and UX
- 15 min is short enough to limit theft damage
- 30 days is long enough for good UX

**Recommendation: Keep these values** unless you have specific requirements.
