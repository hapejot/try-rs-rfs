use clap::Parser;
use clap::Subcommand;
use rfs::{copy_remote_file, NetMsg, NetMsgConnection};
use std::fmt;
use tokio::net::TcpStream;

#[derive(Parser)]
struct Args {
    #[clap(long, short, default_value = "localhost:44444")]
    address: String,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Copy { from: String, to: String },
}

#[derive(Debug)]
enum ClientErr {
    NoServer,
}

impl fmt::Display for ClientErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

async fn run() -> Result<(), ClientErr> {
    let args = Args::parse();

    let stream = TcpStream::connect(args.address)
        .await
        .map_err(|_| ClientErr::NoServer)?;
    let mut con = NetMsgConnection::new(stream);
    con.init();
    let msg = NetMsg::Hello {
        localhost: "localhost".to_string(),
    };

    match args.cmd {
        Command::Copy { from, to } => {
            con.write(msg).await;
            if let Some(_msg) = con.read().await {
                // println!("{:?}", msg);
                copy_remote_file(&mut con, from, to).await;
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => println!("OK"),
        Err(err) => println!("{}", err),
    }
}
