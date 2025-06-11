//! Retry logic and error recovery mechanisms for HTTP requests
//!
//! This module provides:
//! - Exponential backoff strategy for retrying failed requests
//! - Circuit breaker pattern to prevent cascade failures
//! - Configurable retry policies for different error types

use rustacean_docs_core::{error::ErrorContext, Result};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, warn};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Base delay for exponential backoff
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Whether to add jitter to delays
    pub jitter: bool,
    /// Timeout for individual retry attempts
    pub attempt_timeout: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
            attempt_timeout: Duration::from_secs(30),
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected immediately
    Open,
    /// Circuit is half-open, allowing limited requests to test recovery
    HalfOpen,
}

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: usize,
    /// Time to wait before transitioning from Open to HalfOpen
    pub recovery_timeout: Duration,
    /// Number of successful requests needed to close the circuit from HalfOpen
    pub success_threshold: usize,
    /// Time window for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 3,
            failure_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker implementation
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: AtomicUsize, // Using usize to represent CircuitState
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    last_failure_time: AtomicU64,
    last_state_change: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: AtomicUsize::new(CircuitState::Closed as usize),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            last_failure_time: AtomicU64::new(0),
            last_state_change: AtomicU64::new(
                Instant::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            ),
        }
    }

    /// Check if a request should be allowed through the circuit
    pub fn should_allow_request(&self) -> bool {
        let current_state = self.get_state();
        let now = Instant::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        match current_state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let last_change = self.last_state_change.load(Ordering::Relaxed);
                let time_since_open = Duration::from_millis(now - last_change);
                
                if time_since_open >= self.config.recovery_timeout {
                    self.transition_to_half_open();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request
    pub fn record_success(&self) {
        let current_state = self.get_state();
        
        match current_state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if success_count >= self.config.success_threshold {
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {
                // Ignore successes when circuit is open (shouldn't happen)
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        let now = Instant::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.last_failure_time.store(now, Ordering::Relaxed);
        
        let current_state = self.get_state();
        
        match current_state {
            CircuitState::Closed => {
                let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if failure_count >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state transitions back to open
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Already open, just increment counter
                self.failure_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get the current circuit state
    pub fn get_state(&self) -> CircuitState {
        let state_value = self.state.load(Ordering::Relaxed);
        match state_value {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed, // Default fallback
        }
    }

    /// Get circuit breaker statistics
    pub fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            state: self.get_state(),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            last_failure_time: self.last_failure_time.load(Ordering::Relaxed),
        }
    }

    fn transition_to_open(&self) {
        debug!("Circuit breaker transitioning to OPEN state");
        self.state.store(CircuitState::Open as usize, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.update_state_change_time();
    }

    fn transition_to_half_open(&self) {
        debug!("Circuit breaker transitioning to HALF_OPEN state");
        self.state.store(CircuitState::HalfOpen as usize, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.update_state_change_time();
    }

    fn transition_to_closed(&self) {
        debug!("Circuit breaker transitioning to CLOSED state");
        self.state.store(CircuitState::Closed as usize, Ordering::Relaxed);
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.update_state_change_time();
    }

    fn update_state_change_time(&self) {
        let now = Instant::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_state_change.store(now, Ordering::Relaxed);
    }
}

/// Statistics for circuit breaker monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub last_failure_time: u64,
}

/// Retry policy that combines exponential backoff with circuit breaker
#[derive(Debug)]
pub struct RetryPolicy {
    retry_config: RetryConfig,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl RetryPolicy {
    /// Create a new retry policy with default configurations
    pub fn new() -> Self {
        Self {
            retry_config: RetryConfig::default(),
            circuit_breaker: Arc::new(CircuitBreaker::new(CircuitBreakerConfig::default())),
        }
    }

    /// Create a new retry policy with custom configurations
    pub fn with_config(
        retry_config: RetryConfig,
        circuit_breaker_config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            retry_config,
            circuit_breaker: Arc::new(CircuitBreaker::new(circuit_breaker_config)),
        }
    }

    /// Execute a function with retry logic and circuit breaker protection
    pub async fn execute<F, Fut, T, E>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        for attempt in 0..self.retry_config.max_attempts {
            // Check circuit breaker before attempting request
            if !self.circuit_breaker.should_allow_request() {
                warn!("Circuit breaker is OPEN, rejecting request");
                return Err(rustacean_docs_core::error::ErrorBuilder::network().http_request(
                    "Circuit breaker is open - service unavailable", 
                    Some(503)
                ));
            }

            debug!(
                attempt = attempt + 1,
                max_attempts = self.retry_config.max_attempts,
                "Executing operation with retry"
            );

            // Execute the operation with timeout
            let result = tokio::time::timeout(self.retry_config.attempt_timeout, operation()).await;

            match result {
                Ok(Ok(success)) => {
                    debug!("Operation succeeded on attempt {}", attempt + 1);
                    self.circuit_breaker.record_success();
                    return Ok(success);
                }
                Ok(Err(error)) => {
                    warn!(
                        attempt = attempt + 1,
                        error = %error,
                        "Operation failed"
                    );
                    self.circuit_breaker.record_failure();

                    // Don't retry on the last attempt
                    if attempt + 1 >= self.retry_config.max_attempts {
                        return Err(rustacean_docs_core::error::ErrorBuilder::network().http_request(
                            format!("Operation failed after {} attempts: {}", 
                                   self.retry_config.max_attempts, error),
                            None
                        ));
                    }

                    // Calculate delay with exponential backoff
                    let delay = self.calculate_delay(attempt);
                    debug!(
                        delay_ms = delay.as_millis(),
                        "Waiting before retry"
                    );
                    sleep(delay).await;
                }
                Err(_timeout) => {
                    warn!(
                        attempt = attempt + 1,
                        timeout_secs = self.retry_config.attempt_timeout.as_secs(),
                        "Operation timed out"
                    );
                    self.circuit_breaker.record_failure();

                    // Don't retry on the last attempt
                    if attempt + 1 >= self.retry_config.max_attempts {
                        return Err(rustacean_docs_core::error::ErrorBuilder::network().timeout());
                    }

                    // Calculate delay with exponential backoff
                    let delay = self.calculate_delay(attempt);
                    debug!(
                        delay_ms = delay.as_millis(),
                        "Waiting before retry after timeout"
                    );
                    sleep(delay).await;
                }
            }
        }

        // This should never be reached due to the loop logic above
        Err(rustacean_docs_core::error::ErrorBuilder::internal(
            "Retry loop completed unexpectedly"
        ))
    }

    /// Get circuit breaker statistics
    pub fn circuit_breaker_stats(&self) -> CircuitBreakerStats {
        self.circuit_breaker.stats()
    }

    /// Get retry configuration
    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
    }

    /// Calculate delay for the given attempt using exponential backoff
    fn calculate_delay(&self, attempt: usize) -> Duration {
        let delay_ms = (self.retry_config.base_delay.as_millis() as f64
            * self.retry_config.backoff_multiplier.powi(attempt as i32)) as u64;

        let mut delay = Duration::from_millis(delay_ms);

        // Cap the delay at max_delay
        if delay > self.retry_config.max_delay {
            delay = self.retry_config.max_delay;
        }

        // Add jitter if enabled
        if self.retry_config.jitter {
            let jitter_range = delay.as_millis() as f64 * 0.1; // ±10% jitter
            let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
            let jittered_ms = (delay.as_millis() as f64 + jitter).max(0.0) as u64;
            delay = Duration::from_millis(jittered_ms);
        }

        delay
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Determines if an error should be retried
pub fn should_retry_error(error: &rustacean_docs_core::Error) -> bool {
    match error {
        rustacean_docs_core::Error::Network(network_err) => {
            network_err.is_recoverable()
        },
        rustacean_docs_core::Error::Docs(_) => false,
        rustacean_docs_core::Error::Cache(cache_err) => {
            cache_err.is_recoverable()
        },
        rustacean_docs_core::Error::Config(_) => false,
        rustacean_docs_core::Error::Protocol(_) => false,
        rustacean_docs_core::Error::Serialization(_) => false,
        rustacean_docs_core::Error::UrlParse(_) => false,
        rustacean_docs_core::Error::Io(_) => true,
        rustacean_docs_core::Error::Internal(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(10));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.jitter);
        assert_eq!(config.attempt_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, Duration::from_secs(60));
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.failure_window, Duration::from_secs(60));
    }

    #[test]
    fn test_circuit_breaker_initial_state() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
        assert_eq!(breaker.get_state(), CircuitState::Closed);
        assert!(breaker.should_allow_request());
        
        let stats = breaker.stats();
        assert_eq!(stats.state, CircuitState::Closed);
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.success_count, 0);
    }

    #[test]
    fn test_circuit_breaker_failure_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new(config);

        // First failure
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Closed);
        assert!(breaker.should_allow_request());

        // Second failure should open the circuit
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Open);
        assert!(!breaker.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new(config);

        // One failure
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Closed);

        // Success should reset failure count
        breaker.record_success();
        assert_eq!(breaker.get_state(), CircuitState::Closed);

        // Another failure should not open circuit yet
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Closed);
        assert!(breaker.should_allow_request());
    }

    #[tokio::test]
    async fn test_retry_policy_success_on_first_attempt() {
        let policy = RetryPolicy::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let operation = || {
            let counter = Arc::clone(&counter_clone);
            async move {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok::<i32, String>(42)
            }
        };

        let result = policy.execute(operation).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_retry_policy_success_after_failures() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1), // Very short for testing
            ..Default::default()
        };
        let policy = RetryPolicy::with_config(config, CircuitBreakerConfig::default());
        
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let operation = || {
            let counter = Arc::clone(&counter_clone);
            async move {
                let count = counter.fetch_add(1, Ordering::Relaxed);
                if count < 2 {
                    Err("Temporary failure".to_string())
                } else {
                    Ok(42)
                }
            }
        };

        let result = policy.execute(operation).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_retry_policy_max_attempts_exceeded() {
        let config = RetryConfig {
            max_attempts: 2,
            base_delay: Duration::from_millis(1),
            ..Default::default()
        };
        let policy = RetryPolicy::with_config(config, CircuitBreakerConfig::default());
        
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let operation = || {
            let counter = Arc::clone(&counter_clone);
            async move {
                counter.fetch_add(1, Ordering::Relaxed);
                Err::<i32, String>("Always fails".to_string())
            }
        };

        let result = policy.execute(operation).await;
        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_should_retry_error() {
        // Should retry network errors
        let network_error = rustacean_docs_core::error::ErrorBuilder::network().http_request("Connection failed", None);
        assert!(should_retry_error(&network_error));

        // Should retry server errors
        let server_error = rustacean_docs_core::error::ErrorBuilder::network().http_request("Internal server error", Some(500));
        assert!(should_retry_error(&server_error));

        // Should retry rate limiting
        let rate_limit_error = rustacean_docs_core::error::ErrorBuilder::network().rate_limit(None);
        assert!(should_retry_error(&rate_limit_error));

        // Should not retry client errors (except specific ones)
        let client_error = rustacean_docs_core::error::ErrorBuilder::network().http_request("Not found", Some(404));
        assert!(!should_retry_error(&client_error));

        // Should not retry parsing errors
        let parse_error = rustacean_docs_core::error::ErrorBuilder::docs().parse_error("Invalid JSON");
        assert!(!should_retry_error(&parse_error));

        // Should not retry validation errors  
        let validation_error = rustacean_docs_core::error::ErrorBuilder::protocol().invalid_input("test_tool", "Invalid input");
        assert!(!should_retry_error(&validation_error));
    }

    #[test]
    fn test_calculate_delay() {
        let config = RetryConfig {
            base_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(5),
            jitter: false,
            ..Default::default()
        };
        let policy = RetryPolicy::with_config(config, CircuitBreakerConfig::default());

        // First retry (attempt 0)
        let delay0 = policy.calculate_delay(0);
        assert_eq!(delay0, Duration::from_millis(100));

        // Second retry (attempt 1)
        let delay1 = policy.calculate_delay(1);
        assert_eq!(delay1, Duration::from_millis(200));

        // Third retry (attempt 2)
        let delay2 = policy.calculate_delay(2);
        assert_eq!(delay2, Duration::from_millis(400));

        // Should cap at max_delay
        let large_delay = policy.calculate_delay(10);
        assert_eq!(large_delay, Duration::from_secs(5));
    }
}