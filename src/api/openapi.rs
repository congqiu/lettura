//! OpenAPI schema collection.
//!
//! Adding a new endpoint to the public schema is two steps:
//!   1. Add `#[utoipa::path(...)]` above the handler
//!   2. List the handler in the `paths(...)` block on `ApiDoc` below, and
//!      list any new request/response types in `components(schemas(...))`
//!
//! The schema is served at `GET /api/openapi.json`. The frontend regenerates
//! `web/src/api/schema.ts` from it via `pnpm codegen`.
//!
//! Incremental adoption is fine — handlers without `#[utoipa::path]` still
//! work, they just don't appear in the schema. Aim to annotate any handler
//! you're already editing for another reason.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Lettura API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Lettura is a wallabag-inspired read-it-later service. \
                       Endpoints below are stable; unannotated routes exist \
                       but their schema is not yet machine-readable."
    ),
    paths(
        crate::api::health::health_check,
        crate::api::tags::list_tags,
    ),
    components(schemas(
        crate::api::health::HealthResponse,
        crate::models::tag::Tag,
    )),
    tags(
        (name = "health", description = "Service liveness and readiness"),
        (name = "tags", description = "User-defined tags applied to entries"),
    ),
)]
pub struct ApiDoc;

/// Serve the OpenAPI document. No auth — the schema itself does not leak
/// data, only describes shapes.
pub async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema must include the handlers we registered. If somebody adds
    /// a new utoipa::path but forgets to list it in ApiDoc, this catches it.
    #[test]
    fn schema_includes_registered_paths() {
        let doc = ApiDoc::openapi();
        let paths = doc.paths.paths.keys().cloned().collect::<Vec<_>>();
        assert!(paths.contains(&"/api/health".to_string()),
                "/api/health missing from OpenAPI schema, got: {paths:?}");
        assert!(paths.contains(&"/api/v1/tags".to_string()),
                "/api/v1/tags missing from OpenAPI schema, got: {paths:?}");
    }

    /// Schema serializes to valid JSON — what the /api/openapi.json endpoint
    /// will hand to openapi-typescript on the frontend.
    #[test]
    fn schema_serializes_to_json() {
        let doc = ApiDoc::openapi();
        let json = serde_json::to_string(&doc).expect("OpenAPI schema must be JSON-serializable");
        assert!(json.contains("\"HealthResponse\""));
        assert!(json.contains("\"Tag\""));
    }
}
