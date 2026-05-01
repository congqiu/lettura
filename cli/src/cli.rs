use clap::{Parser, Subcommand, ValueEnum};

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
pub enum OutputFormat { Json, Ids, Human }

#[derive(Subcommand)]
pub enum Command {
    Login,
    Whoami,
    Config { #[command(subcommand)] cmd: ConfigCmd },

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

    Tags,
    AuditLogs(AuditLogsArgs),
    Skill { #[command(subcommand)] cmd: SkillCmd },
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
    Install { #[arg(long)] path: Option<String> },
}

#[derive(clap::Args)]
pub struct ListArgs {
    #[arg(long)] pub filter: Option<String>,
    #[arg(long)] pub limit: Option<i64>,
    #[arg(long)] pub fields: Option<String>,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    pub query: String,
    #[arg(long)] pub limit: Option<i64>,
}

#[derive(clap::Args)]
pub struct GetArgs {
    pub id: String,
    #[arg(long, value_enum, default_value = "markdown")]
    pub format: GetFormat,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum GetFormat { Markdown, Json, Html, Text }

#[derive(clap::Args)]
pub struct SaveArgs {
    // Use a distinct clap ID to avoid shadowing the global `--url` flag.
    #[arg(id = "entry_url")]
    pub url: String,
    #[arg(long)] pub title: Option<String>,
    #[arg(long, value_delimiter = ',')] pub tag: Vec<String>,
    #[arg(long)] pub wait: bool,
}

#[derive(clap::Args)]
pub struct TagArgs {
    pub id: Option<String>,
    pub names: Vec<String>,
    #[arg(long, value_delimiter = ',')] pub add: Vec<String>,
    #[arg(long)] pub filter: Option<String>,
    #[arg(long)] pub dry_run: bool,
    #[arg(long)] pub yes: bool,
}

#[derive(clap::Args)]
pub struct UntagArgs {
    pub id: Option<String>,
    pub names: Vec<String>,
    #[arg(long, value_delimiter = ',')] pub remove: Vec<String>,
    #[arg(long)] pub filter: Option<String>,
    #[arg(long)] pub dry_run: bool,
    #[arg(long)] pub yes: bool,
}

#[derive(clap::Args)]
pub struct AuditLogsArgs {
    #[arg(long)] pub action: Option<String>,
    #[arg(long)] pub resource_type: Option<String>,
    #[arg(long)] pub status: Option<String>,
    #[arg(long)] pub limit: Option<i64>,
    #[arg(long)] pub offset: Option<i64>,
}

#[derive(clap::Args)]
pub struct StateChangeArgs {
    pub id: Option<String>,
    #[arg(long)] pub filter: Option<String>,
    #[arg(long)] pub dry_run: bool,
    #[arg(long)] pub yes: bool,
}
