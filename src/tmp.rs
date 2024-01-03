#[derive(Parser)]
struct Args {
    #[clap(long, short, default_value = "localhost:44444")]
    address: String,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Client,
    Server,
}
