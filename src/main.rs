use clap::Parser;

use rfs::server_loop;
use tokio::net::TcpListener;

#[derive(Parser)]
struct Args {
    #[clap(long, short, default_value = "localhost:44444")]
    address: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let listener = TcpListener::bind(args.address).await.unwrap();

    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("new connection from {:#?}", addr);
        tokio::spawn(async move {
            server_loop(socket).await;
        });
    }
}
