use lettura::extract;

fn load_fixture(name: &str) -> String {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path, e))
}

fn load_expected(name: &str) -> serde_json::Value {
    let path = format!("tests/fixtures/{}", name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read expected {}: {}", path, e));
    serde_json::from_str(&content).unwrap()
}

#[test]
fn blog_en_simple_extracts_content() {
    let html = load_fixture("blog_en_simple.html");
    let result = extract::extract(&html, Some("https://example.com/article"), None).unwrap();

    // Metadata verification
    assert_eq!(
        result.title.as_deref(),
        Some("Understanding Rust Ownership")
    );
    assert_eq!(result.author.as_deref(), Some("Jane Smith"));
    assert_eq!(result.language.as_deref(), Some("en"));
    assert_eq!(
        result.preview_image.as_deref(),
        Some("https://example.com/rust.jpg")
    );

    // Content verification: should contain article text
    let expected = load_expected("blog_en_simple.expected.json");

    for phrase in expected["textContent_contains"].as_array().unwrap() {
        let phrase = phrase.as_str().unwrap();
        assert!(
            result.text_content.contains(phrase),
            "Expected text_content to contain: '{}'\nGot: {}",
            phrase,
            &result.text_content[..result.text_content.len().min(500)]
        );
    }

    // Content verification: should NOT contain non-article content
    for phrase in expected["textContent_excludes"].as_array().unwrap() {
        let phrase = phrase.as_str().unwrap();
        assert!(
            !result.text_content.contains(phrase),
            "Expected text_content to NOT contain: '{}'",
            phrase
        );
    }

    // Reading time should be reasonable (1-5 minutes)
    assert!(
        result.reading_time >= 1 && result.reading_time <= 5,
        "reading_time should be 1-5 min, got {}",
        result.reading_time
    );
}

#[test]
fn extract_returns_error_for_garbage_html() {
    let html = "<html><body><div></div></body></html>";
    let result = extract::extract(html, None, None);
    assert!(result.is_err(), "should fail for content-less HTML");
}

#[test]
fn extract_handles_chinese_content() {
    let html = r#"<html lang="zh-CN"><head><title>Rust 所有权机制详解</title></head><body>
        <article>
            <p>Rust 的所有权系统是其最独特的功能之一。它在没有垃圾回收的情况下实现内存安全，使 Rust 程序既快速又安全。在本文中，我们将探讨所有权的三条规则及其如何协同工作以防止常见错误。</p>
            <p>Rust 中的每个值都有一个唯一的所有者。当所有者离开作用域时，该值将被丢弃。所有权可以通过移动来转移，也可以通过引用来共享。这些简单的规则在编译时消除了整类错误，包括使用后释放、双重释放和数据竞争。</p>
        </article>
    </body></html>"#;

    let result = extract::extract(html, None, None).unwrap();
    assert_eq!(result.language.as_deref(), Some("zh-CN"));
    assert!(result.text_content.contains("所有权系统"));
    assert!(result.reading_time >= 1);
}
