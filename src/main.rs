use clap::{Parser, Subcommand};
use hostv1::{
    commands, establish_connection,
    models::{NewVm, VmStatus},
};

#[derive(Parser)]
#[command(version = "1.0")]
#[command(about = "Create and manage VMs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Creates a VM (default cpu 1 memory 500MB)")]
    Create(VmArgs),
    #[command(about = "Lists all VMs")]
    List,
    #[command(about = "Updates a VM by name")]
    Update(VmArgs),
    #[command(about = "Deletes a VM by name")]
    Delete {
        name: String,
    },
    #[command(about = "Runs a stopped vm")]
    Run {
        name: String,
    },
    Stop {
        name: String,
    },
}

#[derive(Debug, Parser)]
struct VmArgs {
    name: String,
    #[arg(long)]
    cpu: Option<i32>,
    #[arg(long)]
    memory: Option<i32>,
}

impl Commands {
    pub fn execute(&self) -> anyhow::Result<()> {
        let mut conn = establish_connection()?;
        match self {
            Commands::Create(args) => {
                let vm = commands::create(
                    &mut conn,
                    NewVm {
                        name: args.name.clone(),
                        cpu: args.cpu.unwrap_or(1),
                        memory: args.memory.unwrap_or(500),
                        status: VmStatus::Stopped,
                    },
                )?;
                println!("created {}", vm.name);
            }
            Commands::Delete { name } => {
                commands::delete(&mut conn, name)?;
                println!("deleted {name}");
            }
            Commands::List => {
                let all = commands::list(&mut conn)?;
                if all.is_empty() {
                    println!("no VMs");
                    return Ok(());
                }
                println!("{:<10}{:<8}{:<10}{}", "NAME", "CPUS", "MEMORY", "STATUS");
                for vm in all {
                    let memory = format!("{} MB", vm.memory);
                    let status = format!("{:?}", vm.status).to_lowercase();
                    println!("{:<10}{:<8}{:<10}{}", vm.name, vm.cpu, memory, status);
                }
            }
            Commands::Update(args) => {
                let vm = commands::update(&mut conn, &args.name, args.cpu, args.memory)?;
                println!("updated {}", vm.name);
            }
            _ => unimplemented!(),
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.command.execute()
}
