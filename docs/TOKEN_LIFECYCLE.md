# Token Lifecycle & Invalidation

## Understanding JWT Behavior

### Why Old Access Tokens Still Work After Refresh

**This is by design and expected behavior.**

```
Timeline:
---------
7:00 PM - Login → Access Token A (expires 7:15 PM)
7:10 PM - Refresh → Access Token B (expires 7:25 PM)
7:12 PM - Use Token A → ✅ STILL WORKS (until 7:15 PM)
7:12 PM - Use Token B → ✅ ALSO WORKS (until 7:25 PM)
7:16 PM - Use Token A → ❌ EXPIRED
7:16 PM - Use Token B → ✅ STILL WORKS (until 7:25 PM)
```

### Why This Is OK

1. **Short-lived** - Token A only works for 5 more minutes
2. **Stateless** - No database lookup needed (fast, scalable)
3. **Industry standard** - This is how OAuth2, Auth0, Firebase, etc. work

### When Tokens ARE Invalidated Immediately

```
Event: User logs out
Action: Increment token_version
Result: ALL access tokens invalidated immediately

Timeline:
---------
7:00 PM - Login → Token (tv=0, expires 7:15 PM)
7:10 PM - Logout → User token_version = 1
7:11 PM - Use Token → ❌ REJECTED (tv=0 != 1)
```

## If You NEED Immediate Invalidation

### Option 1: Token Blacklist (Hybrid Approach)

Add a blacklist for revoked tokens:

```rust
// Add to database
CREATE TABLE token_blacklist (
    jti TEXT PRIMARY KEY,  -- JWT ID
    expires_at TIMESTAMPTZ NOT NULL
);

// Add 'jti' claim to JWT
let claims = Claims {
    sub: user_id.to_string(),
    exp: expiry,
    tv: token_version,
    jti: Uuid::new_v4().to_string(),  // Unique token ID
};

// On refresh, blacklist old token
async fn refresh(...) {
    // ... existing code ...
    
    // Extract jti from old access token and blacklist it
    if let Ok(old_token_data) = decode_jwt(&old_access_token, &cfg) {
        blacklist_token(&state.db, &old_token_data.claims.jti).await?;
    }
    
    // ... return new tokens ...
}

// In middleware, check blacklist
async fn validate_token(...) {
    // ... existing validation ...
    
    // Check if token is blacklisted
    if is_blacklisted(&db, &token_data.claims.jti).await? {
        return Err(AuthenticatedUser::err_unauthorized("Token revoked"));
    }
    
    // ... rest of validation ...
}
```

**Pros:**
- Immediate invalidation on refresh
- Can revoke individual tokens

**Cons:**
- Database lookup on every request (defeats JWT purpose)
- Need to clean up expired blacklist entries
- More complex

### Option 2: Very Short-Lived Tokens

```rust
// Set access token to 1-2 minutes
JWT_EXP_MINUTES=1

// Client refreshes more frequently
// Old tokens expire quickly
```

**Pros:**
- Minimal window for old token use
- Still stateless

**Cons:**
- More refresh requests
- More database load
- Slightly worse UX

### Option 3: Sessions Instead of JWTs

```rust
// Store session in database/Redis
// Every request validates against session store
// Can invalidate immediately
```

**Pros:**
- Immediate invalidation
- Full control

**Cons:**
- Database/Redis lookup on EVERY request
- Not stateless
- Harder to scale

## Current Implementation Analysis

Your current setup:

```rust
✅ Access tokens: 15 minutes (good balance)
✅ Refresh tokens: 30 days (reasonable)
✅ Token rotation: On every refresh (good security)
✅ Token version: Incremented on logout (immediate invalidation)
✅ Hashed storage: Argon2 (secure)
```

### Security Assessment

**Risk Level: LOW**

After refresh, old token works for up to 15 minutes:
- ⚠️ If attacker steals old token: 15 min window
- ✅ If user logs out: Immediate invalidation
- ✅ If token expires: Automatic invalidation
- ✅ If refresh token stolen: Can be revoked

**This is acceptable for most applications.**

## When to Use Each Approach

### Use Current Approach (Stateless JWTs) If:
- ✅ Building a scalable API
- ✅ Need high performance
- ✅ 15-minute window is acceptable
- ✅ Mobile app or SPA
- ✅ Following OAuth2 standards

### Use Token Blacklist If:
- ⚠️ Need immediate invalidation on refresh
- ⚠️ Handling very sensitive data
- ⚠️ Compliance requires it
- ⚠️ Can afford database lookup on every request

### Use Sessions If:
- ⚠️ Building traditional web app
- ⚠️ Need immediate invalidation
- ⚠️ Small scale (< 10k users)
- ⚠️ Don't need mobile app support

## Recommendation

**Keep your current implementation.** It's:
- Industry standard
- Secure enough for most use cases
- Performant and scalable
- Well-tested pattern

**Only add blacklist if:**
- Compliance requires immediate invalidation
- Handling financial/medical data
- Users explicitly request it

## Testing the Behavior

Run the test to see this in action:

```bash
cargo test --test auth_flow -- --nocapture
```

You'll see:
```
✓ Old access token still valid after refresh (expected - JWTs are stateless)
✓ Access token properly invalidated after logout
```

This confirms:
1. Refresh doesn't invalidate old tokens (by design)
2. Logout DOES invalidate all tokens (security feature)
