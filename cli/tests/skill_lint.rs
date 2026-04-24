use clap::Parser;
use lettura_cli::cli::Cli;

/// Skills source file, relative to the cli crate root.
const SKILL_PATH: &str = "../skills/lettura.md";

#[test]
fn every_lettura_cli_command_in_skill_parses() {
    let src = std::fs::read_to_string(SKILL_PATH)
        .unwrap_or_else(|e| panic!("failed to read {SKILL_PATH}: {e}"));
    let examples = extract_cli_examples(&src);
    assert!(
        !examples.is_empty(),
        "no lettura-cli examples found in {SKILL_PATH} — did formatting change?"
    );

    let mut failures: Vec<String> = vec![];
    for (line_no, cmd) in &examples {
        let argv = shell_split(cmd);
        if argv.is_empty() {
            continue;
        }
        // Prepend program name (clap expects argv[0] to be the binary name)
        let mut parse_argv: Vec<String> = vec!["lettura-cli".to_string()];
        parse_argv.extend(argv.into_iter().skip(1)); // drop leading "lettura-cli"

        if let Err(e) = Cli::try_parse_from(&parse_argv) {
            failures.push(format!(
                "line {line_no}: `{cmd}` → parse error: {}",
                e.to_string().lines().next().unwrap_or("?")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "skill markdown contains commands that no longer parse:\n  {}\n\n\
         Fix either the skill ({SKILL_PATH}) or the CLI arg definitions (cli/src/cli.rs).",
        failures.join("\n  ")
    );
}

/// Return (1-based-line-number, command-string) for every line in a code fence
/// (```…```) that starts with "lettura-cli ".
fn extract_cli_examples(src: &str) -> Vec<(usize, String)> {
    let mut out = vec![];
    let mut in_fence = false;
    for (idx, raw) in src.lines().enumerate() {
        let line = raw.trim_start();
        if line.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            continue;
        }
        if let Some(_rest) = line.strip_prefix("lettura-cli") {
            // full command, preserving the "lettura-cli" prefix for shell_split
            // Skip example placeholders like <id> / <tag1> — leave them as-is; clap treats them as values.
            // But skip lines that start with `#` (shell comments).
            out.push((idx + 1, line.to_string()));
        }
    }
    out
}

/// Very small POSIX-ish shell split. Supports double and single quotes.
/// Good enough for the CLI examples we use in the skill.
fn shell_split(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '\\' if !in_single => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ws if ws.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_split_handles_quotes() {
        let v = shell_split(r#"cmd "a b" 'c d' e"#);
        assert_eq!(v, vec!["cmd", "a b", "c d", "e"]);
    }

    #[test]
    fn extract_cli_examples_only_inside_fences() {
        let md = "Outside `lettura-cli save x`.\n\n```\nlettura-cli save https://x\n```\n";
        let got = extract_cli_examples(md);
        assert_eq!(got.len(), 1);
        assert!(got[0].1.starts_with("lettura-cli save"));
    }
}
