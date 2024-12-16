use clap::Parser;

#[derive(Parser)]
enum Commands {
    Open,
    List,
    Withdraw,
    Topup,
    Close,
    ForceClose,
    Info,
}

fn main() {
    let command = Commands::parse();

    match command {
        Commands::Open => {
            println!("Open")
        }
        Commands::List => {
            println!("List")
        }
        Commands::Withdraw => {
            println!("Withdraw")
        }
        Commands::Topup => {
            println!("Topup")
        }
        Commands::Close => {
            println!("Close")
        }
        Commands::ForceClose => {
            println!("ForceClose")
        }
        Commands::Info => {
            println!("Info")
        }
    }
}
