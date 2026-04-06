use std::env;
use std::sync::OnceLock;

/// Authentication configuration — loaded once at startup via `init()`.
#[derive(Clone, Debug)]
pub struct JwtConfig {
    pub secret: String,
    pub access_exp_minutes: i64,
    pub refresh_exp_days: i64,
}

#[derive(Clone, Debug)]
pub struct EnvVariables {
    pub address: String,
    pub port: String,
}

static JWT_CONFIG: OnceLock<JwtConfig> = OnceLock::new();
static ENV_VARIABLES: OnceLock<EnvVariables> = OnceLock::new();

impl JwtConfig {
    /// Call once at application startup (in `main`). Panics if required vars are missing or invalid.
    pub fn init() {
        let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
        if secret.len() < 32 {
            panic!("JWT_SECRET must be at least 32 bytes for security");
        }

        let access_exp_minutes = env::var("JWT_ACCESS_TOKEN_EXPIRATION_MINUTES")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(15);

        let refresh_exp_days = env::var("JWT_REFRESH_TOKEN_EXPIRATION_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(30);

        JWT_CONFIG
            .set(JwtConfig { secret, access_exp_minutes, refresh_exp_days })
            .expect("JwtConfig already initialized");
    }

    /// Get the global config. Panics if `init()` was not called first.
    pub fn get() -> &'static JwtConfig {
        JWT_CONFIG
            .get()
            .expect("JwtConfig not initialized — call JwtConfig::init() at startup")
    }
}

impl EnvVariables {
    pub fn init() {
        let address = env::var("ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

        ENV_VARIABLES
            .set(EnvVariables { address, port })
            .expect("EnvVariables already initialized");
    }

    pub fn get() -> &'static EnvVariables {
        ENV_VARIABLES
            .get()
            .expect("EnvVariables not initialized — call EnvVariables::init() at startup")
    }
}
