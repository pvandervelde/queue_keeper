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
    #[allow(dead_code)]
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

        // Build docker run command.
        // Note: --rm is intentionally omitted so the container is NOT
        // automatically removed on exit.  This preserves logs for debugging
        // when the container crashes during startup.  Cleanup is done
        // explicitly in the Drop impl below.
        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("-d") // Detached
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
        let max_attempts = 60;
        let retry_delay = Duration::from_millis(500);

        for attempt in 1..=max_attempts {
            tokio::time::sleep(retry_delay).await;

            // Detect early container exit: if the process is no longer running
            // there is no point waiting further.
            if let Some(status) = self.exit_status() {
                let logs = self.logs();
                panic!(
                    "Container {} exited unexpectedly after {} health-check attempt(s).\n\
                     Exit status: {}\n\
                     Container logs:\n{}",
                    self.container_id, attempt, status, logs
                );
            }

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

        // Capture container logs before panicking so CI output is actionable.
        let logs = self.logs();
        panic!(
            "Container {} did not become healthy after {} attempts.\n\
             Container logs:\n{}",
            self.container_id, max_attempts, logs
        );
    }

    /// Get the full URL for a path
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Get container logs (both stdout and stderr).
    ///
    /// `docker logs` writes the container's stdout to its own stdout and the
    /// container's stderr to its own stderr.  The Rust `tracing` subscriber
    /// emits to stderr by default, so both streams must be captured to see
    /// service startup errors.
    pub fn logs(&self) -> String {
        let output = Command::new("docker")
            .arg("logs")
            .arg(&self.container_id)
            .output()
            .expect("Failed to run docker logs");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        match (stdout.is_empty(), stderr.is_empty()) {
            (true, true) => "(no output captured — container may have been removed)".to_string(),
            (true, false) => format!("[stderr]\n{}", stderr),
            (false, true) => format!("[stdout]\n{}", stdout),
            (false, false) => format!("[stdout]\n{}\n[stderr]\n{}", stdout, stderr),
        }
    }

    /// Return the container's exit status string if it has already exited,
    /// or `None` if it is still running.
    fn exit_status(&self) -> Option<String> {
        let output = Command::new("docker")
            .args([
                "inspect",
                "--format",
                "{{.State.Status}} {{.State.ExitCode}}",
                &self.container_id,
            ])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout);
        let text = text.trim();
        if text.starts_with("exited") || text.starts_with("dead") {
            Some(text.to_string())
        } else {
            None
        }
    }
}

impl Drop for TestContainer {
    fn drop(&mut self) {
        // Stop and remove container. --rm was not used so we must remove explicitly.
        let _ = Command::new("docker")
            .arg("stop")
            .arg(&self.container_id)
            .output();

        let _ = Command::new("docker")
            .arg("rm")
            .arg("--force")
            .arg(&self.container_id)
            .output();

        println!("Stopped and removed container {}", self.container_id);
    }
}

/// Find an available port on localhost
fn find_available_port() -> u16 {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to find available port");
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
