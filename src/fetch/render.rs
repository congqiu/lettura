//! Optional Chromium-based render fallback, gated behind the `rendering`
//! feature. When the feature is disabled the whole module vanishes from the
//! build and the pipeline skips the render branch entirely.
//!
//! Responsibilities:
//! - Lazy-start a single Chromium process (via `chromiumoxide`) and reuse it
//!   across concurrent fetch workers.
//! - Enforce a concurrency cap with a `Semaphore` so a burst of render
//!   requests can't explode memory.
//! - Track a simple failure circuit breaker — after N consecutive failures the
//!   service enters cooldown and rejects further renders until the window
//!   elapses.

use chromiumoxide::browser::{Browser, BrowserConfig};
use futures_util::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};

/// Number of consecutive render failures that trip the circuit breaker.
const FAILURE_THRESHOLD: usize = 5;
/// How long the breaker stays open once tripped.
const COOLDOWN: Duration = Duration::from_secs(60);

pub struct RenderService {
    sem: Arc<Semaphore>,
    browser: Arc<RwLock<Option<Arc<Browser>>>>,
    failures: AtomicUsize,
    cooldown_until: Mutex<Option<Instant>>,
    chromium_path: Option<String>,
    timeout: Duration,
}

impl RenderService {
    /// Build a service handle. The Chromium process is NOT started here — it is
    /// launched lazily on the first successful `render()` call so the app boots
    /// fast even when no page ever triggers rendering.
    pub fn new(chromium_path: Option<String>, concurrency: usize, timeout_ms: u64) -> Self {
        Self {
            sem: Arc::new(Semaphore::new(concurrency.max(1))),
            browser: Arc::new(RwLock::new(None)),
            failures: AtomicUsize::new(0),
            cooldown_until: Mutex::new(None),
            chromium_path,
            timeout: Duration::from_millis(timeout_ms.max(1000)),
        }
    }

    /// Render a URL and return the final page HTML. Returns `Err` if the
    /// breaker is open, the browser can't be launched, or the page load fails.
    ///
    /// `timeout_override`, when `Some`, takes precedence over the default
    /// timeout set at service construction (per-site rules can cap rendering
    /// time for slow pages).
    pub async fn render(
        &self,
        url: &str,
        wait_for: Option<&str>,
        timeout_override: Option<Duration>,
    ) -> Result<String, String> {
        // Circuit breaker check.
        {
            let mut guard = self.cooldown_until.lock().await;
            if let Some(until) = *guard {
                if Instant::now() < until {
                    return Err("render circuit breaker open".to_string());
                }
                // Cooldown elapsed: clear it and the open-gauge so the dashboard
                // reflects the closed state even if the next attempt fails again.
                *guard = None;
                metrics::gauge!("render_circuit_breaker_open").set(0.0);
            }
        }

        let _permit = self
            .sem
            .acquire()
            .await
            .map_err(|e| format!("render semaphore closed: {}", e))?;

        let browser = self.ensure_browser().await?;

        let timeout = timeout_override.unwrap_or(self.timeout);
        let result = tokio::time::timeout(
            timeout,
            fetch_page_content(&browser, url, wait_for),
        )
        .await
        .map_err(|_| "render timeout".to_string())
        .and_then(|r| r);

        match result {
            Ok(html) => {
                self.failures.store(0, Ordering::Relaxed);
                metrics::gauge!("render_circuit_breaker_open").set(0.0);
                Ok(html)
            }
            Err(e) => {
                self.record_failure().await;
                Err(e)
            }
        }
    }

    /// Graceful shutdown: close the browser if it was started.
    pub async fn shutdown(&self) {
        let mut guard = self.browser.write().await;
        if let Some(browser) = guard.take() {
            if let Some(b) = Arc::into_inner(browser) {
                close_browser(b).await;
            }
        }
    }

    async fn ensure_browser(&self) -> Result<Arc<Browser>, String> {
        {
            let guard = self.browser.read().await;
            if let Some(b) = guard.as_ref() {
                return Ok(b.clone());
            }
        }
        // Lock for write, re-check (double-checked locking).
        let mut guard = self.browser.write().await;
        if guard.is_none() {
            let browser = launch_browser(self.chromium_path.as_deref()).await?;
            *guard = Some(Arc::new(browser));
        }
        Ok(guard.as_ref().expect("browser initialized in double-checked lock").clone())
    }

    async fn record_failure(&self) {
        let n = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= FAILURE_THRESHOLD {
            let mut cooldown_guard = self.cooldown_until.lock().await;
            *cooldown_guard = Some(Instant::now() + COOLDOWN);
            drop(cooldown_guard);
            metrics::gauge!("render_circuit_breaker_open").set(1.0);
            tracing::warn!(
                consecutive_failures = n,
                cooldown_secs = COOLDOWN.as_secs(),
                "render service tripped circuit breaker"
            );
            // Drop the browser so the next attempt after cooldown starts fresh.
            let mut browser_guard = self.browser.write().await;
            if let Some(browser) = browser_guard.take() {
                if let Some(b) = Arc::into_inner(browser) {
                    close_browser(b).await;
                }
            }
            self.failures.store(0, Ordering::Relaxed);
        }
    }
}

async fn close_browser(mut browser: Browser) {
    let _ = browser.close().await;
}

/// Launch Chromium with a fixed set of flags tuned for containerized scraping.
async fn launch_browser(chromium_path: Option<&str>) -> Result<Browser, String> {
    let mut builder = BrowserConfig::builder()
        .no_sandbox()
        .arg("--disable-dev-shm-usage")
        .arg("--disable-gpu")
        .arg("--hide-scrollbars")
        // Reduces memory footprint for throwaway navigations.
        .arg("--disable-background-networking")
        .arg("--disable-default-apps")
        // Stealth-lite: removes the navigator.webdriver automation flag.
        .arg("--disable-blink-features=AutomationControlled");
    if let Some(path) = chromium_path {
        builder = builder.chrome_executable(path);
    }
    let config = builder.build().map_err(|e| format!("browser config: {}", e))?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("browser launch: {}", e))?;

    // Drive the message loop in the background; if it errors we let it die
    // and the next render() will trigger a relaunch.
    tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    tracing::info!("render service: chromium launched");
    Ok(browser)
}

async fn fetch_page_content(
    browser: &Arc<Browser>,
    url: &str,
    wait_for: Option<&str>,
) -> Result<String, String> {
    let page = browser
        .new_page(url)
        .await
        .map_err(|e| format!("new_page: {}", e))?;

    if let Some(sel) = wait_for {
        page.find_element(sel)
            .await
            .map_err(|e| format!("wait_for_selector '{}': {}", sel, e))?;
    } else {
        // Default: wait for the DOMContentLoaded / load event via the navigation future.
        let _ = page.wait_for_navigation().await;
    }

    let html = page
        .content()
        .await
        .map_err(|e| format!("page.content: {}", e))?;
    let _ = page.close().await;
    Ok(html)
}
