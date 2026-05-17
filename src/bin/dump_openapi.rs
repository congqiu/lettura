//! Dump the OpenAPI schema to stdout.
//!
//! Used by the frontend codegen pipeline so `pnpm codegen` can regenerate
//! `web/src/api/schema.ts` without needing a running server.
//!
//! Usage:
//!   cargo run --bin dump-openapi --no-default-features > web/src/api/openapi.json
//!   # then in web/:
//!   pnpm codegen

use utoipa::OpenApi;

fn main() {
    let doc = lettura::api::openapi::ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&doc)
        .expect("OpenAPI schema must serialize as JSON");
    println!("{json}");
}
