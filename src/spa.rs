use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::{Embed, EmbeddedFile};

#[derive(Embed)]
#[folder = "web/dist"]
struct Asset;

fn cache_control_for(path: &str) -> &'static str {
    if path == "sw.js"
        || path == "registerSW.js"
        || path.starts_with("workbox-")
        || path == "manifest.webmanifest"
        || path == "index.html"
    {
        "no-cache"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=86400"
    }
}

fn is_static_asset_path(path: &str) -> bool {
    let ext = match path.rsplit('.').next() {
        Some(ext) if ext != path => ext,
        _ => return false,
    };
    matches!(
        ext,
        "js" | "mjs"
            | "css"
            | "map"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "svg"
            | "ico"
            | "woff"
            | "woff2"
            | "ttf"
            | "webmanifest"
            | "json"
            | "txt"
            | "xml"
    )
}

fn etag_of(file: &EmbeddedFile) -> String {
    let hash = file.metadata.sha256_hash();
    format!("\"{}\"", hex::encode(hash))
}

fn build_asset_response(
    path: &str,
    file: EmbeddedFile,
    req_headers: &HeaderMap,
) -> Response {
    let etag = etag_of(&file);
    let cache_control = cache_control_for(path);

    if let Some(if_none_match) = req_headers.get(header::IF_NONE_MATCH) {
        if if_none_match.as_bytes() == etag.as_bytes() {
            let mut resp = Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .body(Body::empty())
                .expect("NOT_MODIFIED response is always valid");
            let h = resp.headers_mut();
            h.insert(header::ETAG, HeaderValue::from_str(&etag).expect("ETag is valid ASCII"));
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static(cache_control),
            );
            return resp;
        }
    }

    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(file.data.into_owned()))
        .expect("OK response with body is always valid");
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref()).expect("MIME type is valid ASCII"),
    );
    h.insert(header::ETAG, HeaderValue::from_str(&etag).expect("ETag is valid ASCII"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    resp
}

pub async fn spa_handler(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(file) = Asset::get(path) {
        return build_asset_response(path, file, &headers);
    }

    if is_static_asset_path(path) {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    match Asset::get("index.html") {
        Some(file) => build_asset_response("index.html", file, &headers),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_control_for_sw_js_is_no_cache() {
        assert_eq!(cache_control_for("sw.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_register_sw_js_is_no_cache() {
        assert_eq!(cache_control_for("registerSW.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_workbox_runtime_is_no_cache() {
        assert_eq!(cache_control_for("workbox-abc123.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_manifest_is_no_cache() {
        assert_eq!(cache_control_for("manifest.webmanifest"), "no-cache");
    }

    #[test]
    fn cache_control_for_index_html_is_no_cache() {
        assert_eq!(cache_control_for("index.html"), "no-cache");
    }

    #[test]
    fn cache_control_for_hashed_asset_is_immutable() {
        assert_eq!(
            cache_control_for("assets/index-abc123.js"),
            "public, max-age=31536000, immutable"
        );
    }

    #[test]
    fn cache_control_for_favicon_is_one_day() {
        assert_eq!(cache_control_for("favicon.svg"), "public, max-age=86400");
    }

    #[test]
    fn cache_control_for_pwa_png_is_one_day() {
        assert_eq!(
            cache_control_for("pwa-512x512.png"),
            "public, max-age=86400"
        );
    }

    #[test]
    fn is_static_asset_js() {
        assert!(is_static_asset_path("sw.js"));
        assert!(is_static_asset_path("assets/index-abc.js"));
    }

    #[test]
    fn is_static_asset_mjs() {
        assert!(is_static_asset_path("assets/foo.mjs"));
    }

    #[test]
    fn is_static_asset_css() {
        assert!(is_static_asset_path("assets/style-xyz.css"));
    }

    #[test]
    fn is_static_asset_webmanifest() {
        assert!(is_static_asset_path("manifest.webmanifest"));
    }

    #[test]
    fn is_static_asset_source_map() {
        assert!(is_static_asset_path("sw.js.map"));
    }

    #[test]
    fn is_static_asset_images_and_fonts() {
        for p in [
            "pwa-512x512.png",
            "icon.jpg",
            "pic.jpeg",
            "anim.gif",
            "modern.webp",
            "favicon.svg",
            "favicon.ico",
            "Inter.woff",
            "Inter.woff2",
            "Noto.ttf",
        ] {
            assert!(is_static_asset_path(p), "expected {p} to be static asset");
        }
    }

    #[test]
    fn is_static_asset_json_and_txt_and_xml() {
        assert!(is_static_asset_path("data.json"));
        assert!(is_static_asset_path("robots.txt"));
        assert!(is_static_asset_path("sitemap.xml"));
    }

    #[test]
    fn is_not_static_asset_bare_route() {
        assert!(!is_static_asset_path("articles/xyz"));
        assert!(!is_static_asset_path("login"));
        assert!(!is_static_asset_path(""));
    }

    #[test]
    fn is_not_static_asset_unknown_extension() {
        assert!(!is_static_asset_path("foo.bar"));
    }
}