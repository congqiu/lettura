use crate::api_types::EntryFull;
use crate::cli::{GetArgs, GetFormat};
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::emit_json;

pub async fn run(client: &ApiClient, args: &GetArgs) -> Result<i32, CliError> {
    let entry: EntryFull = client
        .get(&format!("/api/v1/entries/{}", args.id), &[])
        .await?;
    match args.format {
        GetFormat::Json => emit_json(&entry, true).map_err(CliError::from)?,
        GetFormat::Html => {
            println!("{}", entry.content.clone().unwrap_or_default());
        }
        GetFormat::Text => {
            let text = entry
                .content
                .as_deref()
                .map(|h| html2text::from_read(h.as_bytes(), text_width()))
                .unwrap_or_default();
            print!("{text}");
        }
        GetFormat::Markdown => {
            println!("---");
            println!("id: {}", entry.id);
            println!("url: {}", entry.url);
            if let Some(t) = &entry.title {
                println!("title: {t}");
            }
            println!("tags: {:?}", entry.tags);
            if let Some(d) = &entry.created_at {
                println!("saved_at: {d}");
            }
            println!("---\n");
            let md = entry
                .content
                .as_deref()
                .map(html_to_markdown)
                .unwrap_or_default();
            print!("{md}");
        }
    }
    Ok(0)
}

fn html_to_markdown(html: &str) -> String {
    html2text::from_read(html.as_bytes(), text_width())
}

fn text_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
        .max(40)
}
