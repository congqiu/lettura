//! Fetch pipeline: rule-based static fetch with optional render fallback.
//!
//! This module replaces the monolithic logic previously in `tasks/fetcher.rs`.
//! Submodules are kept small and individually testable:
//! - `rewrite`      — URL path rewriting driven by YAML site config
//! - `json_extract` — JSON Pointer-based extraction for API responses
//! - `http`         — HTTP fetch with site-config-driven headers/cookies/UA
//! - `pipeline`     — top-level orchestration (added in a later task)
//! - `render`       — Chromium-based fallback (feature-gated)

pub mod http;
pub mod json_extract;
pub mod pipeline;
#[cfg(feature = "rendering")]
pub mod render;
pub mod rewrite;
