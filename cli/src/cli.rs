use clap::{Parser, Subcommand, ValueEnum, Args};

#[derive(Parser)]
#[command(name = "lettura-cli", version, about = "AI-first CLI for Lettura")]
pub struct Cli {
    #[arg(long, global = true, env = "LETTURA_PROFILE")]
    pub profile: Option<String>,
    #[arg(long, global = true, env = "LETTURA_URL")]
    pub url: Option<String>,
    #[arg(long, global = true, env = "LETTURA_TOKEN", hide_env_values = true)]
    pub token: Option<String>,
    #[arg(long, global = true, value_enum, default_value = "json")]
    pub output: OutputFormat,
    #[arg(long, global = true)]
    pub quiet: bool,
    #[arg(long, global = true)]
    pub pretty: bool,
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    Ids,
    Human,
}

#[derive(Subcommand)]
pub enum Command {
    Login,
    Whoami,
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },

    List(ListArgs),
    Search(SearchArgs),
    Get(GetArgs),

    Save(SaveArgs),
    Tag(TagArgs),
    Untag(UntagArgs),
    Archive(StateChangeArgs),
    Unarchive(StateChangeArgs),
    Star(StateChangeArgs),
    Unstar(StateChangeArgs),

    /// Manage published pages
    Pages {
        #[command(subcommand)]
        cmd: PagesCmd,
    },

    Tags,
    AuditLogs(AuditLogsArgs),
    Skill {
        #[command(subcommand)]
        cmd: SkillCmd,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    Get { key: String },
    Set { key: String, value: String },
    List,
}

#[derive(Subcommand)]
pub enum SkillCmd {
    Print,
    Install {
        #[arg(long)]
        path: Option<String>,
    },
}

#[derive(clap::Args)]
pub struct ListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long)]
    pub limit: Option<i64>,
    #[arg(long)]
    pub fields: Option<String>,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    pub query: String,
    #[arg(long)]
    pub limit: Option<i64>,
}

#[derive(clap::Args)]
pub struct GetArgs {
    pub id: String,
    #[arg(long, value_enum, default_value = "markdown")]
    pub format: GetFormat,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum GetFormat {
    Markdown,
    Json,
    Html,
    Text,
}

#[derive(clap::Args)]
pub struct SaveArgs {
    // Use a distinct clap ID to avoid shadowing the global `--url` flag.
    #[arg(id = "entry_url")]
    pub url: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub tag: Vec<String>,
    #[arg(long)]
    pub wait: bool,
}

#[derive(clap::Args)]
pub struct TagArgs {
    pub id: Option<String>,
    pub names: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    pub add: Vec<String>,
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub yes: bool,
}

#[derive(clap::Args)]
pub struct UntagArgs {
    pub id: Option<String>,
    pub names: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    pub remove: Vec<String>,
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub yes: bool,
}

#[derive(clap::Args)]
pub struct AuditLogsArgs {
    #[arg(long)]
    pub action: Option<String>,
    #[arg(long)]
    pub resource_type: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub limit: Option<i64>,
    #[arg(long)]
    pub offset: Option<i64>,
}

#[derive(clap::Args)]
pub struct StateChangeArgs {
    pub id: Option<String>,
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Subcommand)]
pub enum PagesCmd {
    /// Publish a page from a local file, directory, or URL
    Publish(PagesPublishArgs),
    /// List published pages
    List(PagesListArgs),
    /// Update a page
    Update(PagesUpdateArgs),
    /// Delete a page
    Delete(PagesDeleteArgs),
    /// Restore a deleted page
    Restore(PagesRestoreArgs),
    /// Get share URL for a page
    Share(PagesShareArgs),
}

#[derive(Args)]
pub struct PagesPublishArgs {
    /// Local file/directory path or remote URL
    pub source: String,

    /// Page title (default: extracted from HTML or filename)
    #[arg(long)]
    pub title: Option<String>,

    /// Page description
    #[arg(long)]
    pub description: Option<String>,

    /// Entry HTML file (default: index.html)
    #[arg(long)]
    pub entry_file: Option<String>,

    /// Access password
    #[arg(long)]
    pub password: Option<String>,

    /// Expiration time (RFC 3339, e.g. 2026-12-31T23:59:59Z)
    #[arg(long)]
    pub expires_at: Option<String>,
}

#[derive(Args)]
pub struct PagesListArgs {
    /// Filter by status
    #[arg(long, default_value = "active")]
    pub status: String,

    /// Page number
    #[arg(long, default_value = "1")]
    pub page: u32,

    /// Items per page
    #[arg(long, default_value = "20")]
    pub limit: u32,
}

#[derive(Args)]
pub struct PagesUpdateArgs {
    /// Page ID
    pub id: String,

    /// Update title
    #[arg(long)]
    pub title: Option<String>,

    /// Update description
    #[arg(long)]
    pub description: Option<String>,

    /// Set or change access password
    #[arg(long)]
    pub password: Option<String>,

    /// Clear access password
    #[arg(long)]
    pub clear_password: bool,

    /// Update status
    #[arg(long)]
    pub status: Option<String>,

    /// Update expiration time ("none" to clear)
    #[arg(long)]
    pub expires_at: Option<String>,

    /// Replace page files (local path or URL)
    #[arg(long)]
    pub files: Option<String>,

    /// Update entry HTML file
    #[arg(long)]
    pub entry_file: Option<String>,
}

#[derive(Args)]
pub struct PagesDeleteArgs {
    /// Page ID
    pub id: String,
}

#[derive(Args)]
pub struct PagesRestoreArgs {
    /// Page ID
    pub id: String,
}

#[derive(Args)]
pub struct PagesShareArgs {
    /// Page ID
    pub id: String,
}
