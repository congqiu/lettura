mod cli;

use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let code = run(args).await;
    std::process::exit(code);
}

async fn run(args: Cli) -> i32 {
    match args.cmd {
        Command::Login | Command::Whoami | Command::Config { .. } => todo!("phase 4"),
        Command::List(_) | Command::Search(_) | Command::Get(_) | Command::Tags => todo!("phase 5"),
        Command::Save(_) | Command::Tag(_) | Command::Untag(_) | Command::Archive(_) | Command::Star(_) => todo!("phase 6"),
        Command::Skill { .. } => todo!("phase 7"),
    }
}
