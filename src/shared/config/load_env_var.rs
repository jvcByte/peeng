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
    pub db_url: String,
}

static JWT_CONFIG: OnceLock<JwtConfig> = OnceLock::new();
static ENV_VARIABLES: OnceLock<EnvVariables> = OnceLock::new();

impl JwtConfig {
    /// Call once at application startup (in `main`). Panics if required vars are missing.
    pub fn init() {
        let secret = env::var("JWT_SECRET").expect(".env: JWT_SECRET must be set");

        let access_exp_minutes = match env::var("JWT_EXP_MINUTES") {
            Ok(v) => v.parse::<i64>().unwrap_or_else(|_| {
                eprintln!("WARNING: Invalid JWT_EXP_MINUTES, defaulting to 15");
                15
            }),
            Err(_) => 15,
        };

        let refresh_exp_days = match env::var("REFRESH_TOKEN_EXP_DAYS") {
            Ok(v) => v.parse::<i64>().unwrap_or_else(|_| {
                eprintln!("WARNING: Invalid REFRESH_TOKEN_EXP_DAYS, defaulting to 30");
                30
            }),
            Err(_) => 30,
        };

        JWT_CONFIG
            .set(JwtConfig {
                secret,
                access_exp_minutes,
                refresh_exp_days,
            })
            .expect("JwtConfig already initialized");
    }

    /// Get the global config. Panics if `init()` was not called first.
    pub fn get() -> &'static JwtConfig {
        JWT_CONFIG
            .get()
            .expect("JwtConfig not initialized — call AuthConfig::init() at startup")
    }
}

impl EnvVariables {
    pub fn init() {
        let db_url = env::var("DATABASE_URL").expect(".env: DATABASE_URL must be set");
        let address = env::var("ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

        ENV_VARIABLES
            .set(EnvVariables {
                db_url,
                address,
                port,
            })
            .expect("EnvVariables already initialized");
    }

    pub fn get() -> &'static EnvVariables {
        ENV_VARIABLES
            .get()
            .expect("EnvVariables not initialized — call EnvVariables::init() at startup")
    }
}
