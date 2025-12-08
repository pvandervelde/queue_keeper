//! Common utilities for end-to-end tests
//!
//! These utilities manage Docker containers and make HTTP requests
//! to test the deployed service.
//!
//! **Prerequisites**: Docker image `queue-keeper:test` must be built before running tests.

use std::process::{Command, Stdio};
use std::time::Duration;

/// Test container handle that automatically cleans up on drop
pub struct TestContainer {
    pub container_id: String,
    pub base_url: String,
    pub port: u16,
}

impl TestContainer {
    /// Start a container from the queue-keeper:test image
    pub async fn start() -> Self {
        Self::start_with_env(Vec::new()).await
    }

    /// Start a container with custom environment variables
    pub async fn start_with_env(env_vars: Vec<(&str, &str)>) -> Self {
        // Find an available port
        let port = find_available_port();

        // Build docker run command
        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("-d") // Detached
            .arg("--rm") // Remove on exit
            .arg("-p")
            .arg(format!("{}:8080", port)); // Map container port 8080 to host port

        // Add environment variables
        for (key, value) in env_vars {
            cmd.arg("-e").arg(format!("{}={}", key, value));
        }

        cmd.arg("queue-keeper:test");

        // Start container
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to start Docker container. Ensure Docker is running and queue-keeper:test image exists.");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("Failed to start container: {}", stderr);
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let base_url = format!("http://localhost:{}", port);

        let container = Self {
            container_id: container_id.clone(),
            base_url,
            port,
        };

        // Wait for container to be healthy
        container.wait_for_health().await;

        container
    }

    /// Wait for the container to become healthy
    async fn wait_for_health(&self) {
        let client = http_client();
        let health_url = format!("{}/health", self.base_url);
        let max_attempts = 30;
        let retry_delay = Duration::from_millis(500);

        for attempt in 1..=max_attempts {
            tokio::time::sleep(retry_delay).await;

            if let Ok(response) = client.get(&health_url).send().await {
                if response.status().is_success() {
                    println!(
                        "Container {} is healthy after {} attempts",
                        self.container_id, attempt
                    );
                    return;
                }
            }
        }

        panic!(
            "Container {} did not become healthy after {} attempts",
            self.container_id, max_attempts
        );
    }

    /// Get the full URL for a path
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Get container logs
    pub fn logs(&self) -> String {
        let output = Command::new("docker")
            .arg("logs")
            .arg(&self.container_id)
            .output()
            .expect("Failed to get container logs");

        String::from_utf8_lossy(&output.stdout).to_string()
    }
}

impl Drop for TestContainer {
    fn drop(&mut self) {
        // Stop and remove container
        let _ = Command::new("docker")
            .arg("stop")
            .arg(&self.container_id)
            .output();

        println!("Stopped container {}", self.container_id);
    }
}

/// Find an available port on localhost
fn find_available_port() -> u16 {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to find available port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Create an HTTP client for testing
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

/// Create valid GitHub webhook headers for testing
pub fn github_webhook_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-github-event", "pull_request".parse().unwrap());
    headers.insert(
        "x-github-delivery",
        "12345678-1234-1234-1234-123456789012".parse().unwrap(),
    );
    headers.insert(
        "x-hub-signature-256",
        "sha256=test_signature".parse().unwrap(),
    );
    headers.insert("content-type", "application/json".parse().unwrap());
    headers
}

/// Sample webhook payload for testing
pub fn sample_webhook_payload() -> serde_json::Value {
    serde_json::json!({
        "action": "opened",
        "number": 123,
        "pull_request": {
            "id": 1,
            "number": 123,
            "title": "Test PR",
            "state": "open"
        },
        "repository": {
            "id": 1,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": {
                "login": "owner",
                "id": 1,
                "type": "User"
            }
        }
    })
}
