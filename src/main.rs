use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version = "1.0")]
#[command(about = "Create and manage VMS", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Creates a VM default cpu 1 memory 500MB")]
    Create(CreateVm),
    List,
    Update(CreateVm),
    Delete {
        name: String,
    },
    // Stop
    // Run
}

#[derive(Debug, Parser)]
struct CreateVm {
    name: String,
    #[arg(long)]
    cpu: Option<u32>,
    #[arg(long)]
    memory: Option<u64>,
}

impl Commands {
    pub fn execute(&self) {
        match &self {
            Commands::Create(args) => {
                todo!();
            }
            Commands::Delete { name } => {
                todo!();
            }
            Commands::List => {
                todo!();
            }
            Commands::Update(args) => {
                todo!();
            }
        }
    }
}
fn main() {
    let cli = Cli::parse();
    let command = cli.command.as_ref();
    // command.and_then(|c|c.execute());
}
