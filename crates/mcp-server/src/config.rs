use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub server: ServerSettings,
    pub client: ClientSettings,
    pub cache: CacheSettings,
    pub logging: LoggingSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub name: String,
    pub version: String,
    pub description: String,
    pub bind_address: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSettings {
    pub user_agent: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettings {
    pub memory_max_entries: usize,
    pub memory_ttl_secs: u64,
    pub disk_enabled: bool,
    pub disk_max_size_mb: u64,
    pub disk_ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub format: String,
}


impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            name: "rustacean-docs-mcp".to_string(),
            version: "0.1.0".to_string(),
            description: "MCP server for Rust documentation access".to_string(),
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            user_agent: "rustacean-docs-mcp/0.1.0".to_string(),
            timeout_secs: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
            base_url: "https://docs.rs".to_string(),
        }
    }
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            memory_max_entries: 1000,
            memory_ttl_secs: 3600, // 1 hour
            disk_enabled: true,
            disk_max_size_mb: 500,
            disk_ttl_secs: 86400, // 24 hours
        }
    }
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut config = Config::default();

        // Override with environment variables if present
        config.load_from_env()?;

        Ok(config)
    }

    fn load_from_env(&mut self) -> Result<()> {
        // Server settings
        if let Ok(name) = env::var("RUSTACEAN_DOCS_SERVER_NAME") {
            self.server.name = name;
        }
        if let Ok(version) = env::var("RUSTACEAN_DOCS_SERVER_VERSION") {
            self.server.version = version;
        }
        if let Ok(bind_address) = env::var("RUSTACEAN_DOCS_BIND_ADDRESS") {
            self.server.bind_address = bind_address;
        }
        if let Ok(port) = env::var("RUSTACEAN_DOCS_PORT") {
            self.server.port = port.parse()?;
        }

        // Client settings
        if let Ok(user_agent) = env::var("RUSTACEAN_DOCS_USER_AGENT") {
            self.client.user_agent = user_agent;
        }
        if let Ok(timeout) = env::var("RUSTACEAN_DOCS_CLIENT_TIMEOUT") {
            self.client.timeout_secs = timeout.parse()?;
        }
        if let Ok(base_url) = env::var("RUSTACEAN_DOCS_BASE_URL") {
            self.client.base_url = base_url;
        }

        // Cache settings
        if let Ok(max_entries) = env::var("RUSTACEAN_DOCS_CACHE_MAX_ENTRIES") {
            self.cache.memory_max_entries = max_entries.parse()?;
        }
        if let Ok(ttl) = env::var("RUSTACEAN_DOCS_CACHE_TTL") {
            self.cache.memory_ttl_secs = ttl.parse()?;
        }
        if let Ok(disk_enabled) = env::var("RUSTACEAN_DOCS_CACHE_DISK_ENABLED") {
            self.cache.disk_enabled = disk_enabled.parse()?;
        }

        // Logging settings
        if let Ok(level) = env::var("RUSTACEAN_DOCS_LOG_LEVEL") {
            self.logging.level = level;
        }
        if let Ok(format) = env::var("RUSTACEAN_DOCS_LOG_FORMAT") {
            self.logging.format = format;
        }

        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        // Validate server settings
        if self.server.name.is_empty() {
            return Err(anyhow::anyhow!("Server name cannot be empty"));
        }
        if self.server.port == 0 {
            return Err(anyhow::anyhow!("Server port must be greater than 0"));
        }

        // Validate client settings
        if self.client.user_agent.is_empty() {
            return Err(anyhow::anyhow!("User agent cannot be empty"));
        }
        if self.client.timeout_secs == 0 {
            return Err(anyhow::anyhow!("Client timeout must be greater than 0"));
        }

        // Validate cache settings
        if self.cache.memory_max_entries == 0 {
            return Err(anyhow::anyhow!("Cache max entries must be greater than 0"));
        }

        // Validate logging settings
        match self.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(anyhow::anyhow!("Invalid log level: {}", self.logging.level)),
        }

        Ok(())
    }
}
