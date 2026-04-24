use clap::Parser;
use lettura_cli::cli::{Cli, Command};
use lettura_cli::commands;
use lettura_cli::config::{Config, Override, resolve};
use lettura_cli::error::{emit_error_to_stderr, CliError};

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let code = run(args).await;
    std::process::exit(code);
}

async fn run(args: Cli) -> i32 {
    let result: Result<i32, CliError> = match &args.cmd {
        Command::Login => commands::login::run(args.profile.as_deref()).await,
        Command::Whoami => {
            match load_resolved(&args) {
                Ok(r) => commands::whoami::run(&r).await,
                Err(e) => Err(e),
            }
        }
        Command::Config { cmd } => commands::config::run(cmd),
        _ => {
            eprintln!("command not yet implemented");
            return 4;
        }
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
