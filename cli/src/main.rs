use clap::Parser;
use lettura_cli::cli::{Cli, Command};
use lettura_cli::client::ApiClient;
use lettura_cli::commands;
use lettura_cli::config::{Config, Override, resolve};
use lettura_cli::error::{CliError, emit_error_to_stderr};

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    lettura_cli::output::set_quiet(args.quiet);
    let code = run(args).await;
    std::process::exit(code);
}

macro_rules! with_client {
    ($args:expr, $call:expr) => {
        match resolved_client($args) {
            Ok(c) => $call(&c).await,
            Err(e) => Err(e),
        }
    };
}

async fn run(args: Cli) -> i32 {
    let result: Result<i32, CliError> = match &args.cmd {
        Command::Login => commands::login::run(args.profile.as_deref()).await,
        Command::Whoami => with_client!(&args, commands::whoami::run),
        Command::Config { cmd } => commands::config::run(cmd),

        Command::List(list_args) => with_client!(&args, |c| {
            commands::list::run(c, list_args, args.output, args.pretty)
        }),
        Command::Search(search_args) => with_client!(&args, |c| {
            commands::search::run(c, search_args, args.output, args.pretty)
        }),
        Command::Get(get_args) => with_client!(&args, |c| commands::get::run(c, get_args)),
        Command::Save(save_args) => with_client!(&args, |c| commands::save::run(c, save_args)),
        Command::Tags => with_client!(&args, |c| commands::tags::run(c, args.output, args.pretty)),
        Command::AuditLogs(audit_args) => with_client!(&args, |c| {
            commands::audit_logs::run(c, audit_args, args.output, args.pretty)
        }),
        Command::Tag(tag_args) => with_client!(&args, |c| commands::tag::run_tag(c, tag_args)),
        Command::Untag(untag_args) => {
            with_client!(&args, |c| commands::tag::run_untag(c, untag_args))
        }
        Command::Archive(sc_args) => {
            with_client!(&args, |c| commands::state::run_archive(c, sc_args))
        }
        Command::Unarchive(sc_args) => {
            with_client!(&args, |c| commands::state::run_unarchive(c, sc_args))
        }
        Command::Star(sc_args) => with_client!(&args, |c| commands::state::run_star(c, sc_args)),
        Command::Unstar(sc_args) => {
            with_client!(&args, |c| commands::state::run_unstar(c, sc_args))
        }

        Command::Skill { cmd } => match cmd {
            lettura_cli::cli::SkillCmd::Print => commands::skill::run_print(),
            lettura_cli::cli::SkillCmd::Install { path } => {
                commands::skill::run_install(path.as_deref())
            }
        },
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            emit_error_to_stderr(&e);
            e.exit_code() as i32
        }
    }
}

fn load_resolved(args: &Cli) -> Result<lettura_cli::config::Resolved, CliError> {
    let path = Config::default_path().map_err(|e| CliError::BadArgs(e.to_string()))?;
    let cfg = Config::load_from(&path).unwrap_or_default();
    let over = Override {
        profile: args.profile.clone(),
        url: args.url.clone(),
        token: args.token.clone(),
    };
    resolve(&cfg, &over).map_err(|e| CliError::BadArgs(e.to_string()))
}

fn resolved_client(args: &Cli) -> Result<ApiClient, CliError> {
    let resolved = load_resolved(args)?;
    ApiClient::new(resolved.url, &resolved.token).map_err(CliError::from)
}
