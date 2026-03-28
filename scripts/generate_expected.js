// Usage: node scripts/generate_expected.js tests/fixtures/blog_en_simple.html
// Requires: npm install @mozilla/readability jsdom
const { Readability } = require("@mozilla/readability");
const { JSDOM } = require("jsdom");
const fs = require("fs");

const filePath = process.argv[2];
if (!filePath) {
    console.error("Usage: node generate_expected.js <html-file>");
    process.exit(1);
}

const html = fs.readFileSync(filePath, "utf-8");
const doc = new JSDOM(html, { url: "https://example.com/article" });
const reader = new Readability(doc.window.document);
const article = reader.parse();

if (!article) {
    console.error("Readability failed to parse");
    process.exit(1);
}

const output = {
    title: article.title,
    textContent: article.textContent.trim(),
    excerpt: article.excerpt,
    byline: article.byline,
    length: article.length,
};

const outPath = filePath.replace(".html", ".expected.json");
fs.writeFileSync(outPath, JSON.stringify(output, null, 2));
console.log(`Written to ${outPath}`);
