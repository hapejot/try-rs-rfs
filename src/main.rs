use clap::{clap_derive::Subcommand, Parser};
use serde::{Deserialize, Serialize};
use std::{borrow::BorrowMut, fs::OpenOptions, future::Future, io::Write, process::Output};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{tcp::ReadHalf, tcp::WriteHalf, TcpListener, TcpStream},
};

#[cfg(windows)]
use std::os::windows::prelude::*;
#[cfg(unix)]
use std::os::unix::prelude::*;

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

#[derive(Serialize, Deserialize, Debug)]
pub enum NetMsg {
    Hello {
        localhost: String,
    },
    Message {
        text: String,
    },
    OpenRequest {
        name: String,
    },
    OpenResponse {
        handle: usize,
    },
    ReadRequest {
        handle: usize,
        start: usize,
        len: usize,
    },
    ReadResponse {
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
}

pub struct NetMsgConnection {
    stream: TcpStream,
}

impl NetMsgConnection {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn init(&mut self) {}

    pub async fn write(&mut self, msg: NetMsg) {
        let bytes: Vec<u8> = serde_xdr::to_bytes(&msg).unwrap();

        self.stream.write_all(bytes.as_slice()).await.unwrap();
        self.stream.flush().await.unwrap();
    }

    pub async fn read(&mut self) -> Option<NetMsg> {
        let mut bbuf = bytebuffer::ByteBuffer::new();
        let mut buf = [0u8; 5000];
        loop {
            let bytes_read = self.stream.read(&mut buf).await.unwrap();

            if bytes_read == 0 {
                return None;
            }
            // if bbuf.len() > 0 {
            //     println!(".... additional {}", bytes_read);
            // }
            bbuf.write(&buf[..bytes_read]).unwrap();

            match serde_xdr::from_bytes::<_, NetMsg>(bbuf.as_bytes()) {
                Ok(msg) => break Some(msg),
                Err(_) => {
                    // println!("continue reading.. {}", bytes_read);
                    continue;
                }
            }
        }
    }
}

async fn copy_remote_file(
    con: &mut NetMsgConnection,
    remote_file_name: String,
    local_file_name: String,
) {
    con.write(NetMsg::OpenRequest {
        name: remote_file_name,
    })
    .await;

    match con.read().await {
        Some(NetMsg::OpenResponse { handle }) => {
            let mut f = OpenOptions::new()
                .write(true)
                .append(false)
                .create(true)
                .truncate(true)
                .open(local_file_name)
                .unwrap();
            let mut n = 0;

            loop {
                con.write(NetMsg::ReadRequest {
                    handle,
                    start: n,
                    len: 4000,
                })
                .await;
                match con.read().await {
                    Some(NetMsg::ReadResponse { data }) => {
                        if data.len() == 0 {
                            break;
                        }
                        f.write(data.as_slice()).unwrap();
                        n += data.len();
                    }
                    others => {
                        println!("read error: {:?}", others);
                        break;
                    }
                };
            }
        }
        others => {
            println!("open error: {:?}", others);
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.cmd {
        Command::Server => {
            let listener = TcpListener::bind(args.address).await.unwrap();

            loop {
                let (socket, addr) = listener.accept().await.unwrap();
                println!("new connection from {:#?}", addr);

                tokio::spawn(async move {
                    server_loop(socket).await;
                });
            }
        }

        Command::Client => {
            let stream = TcpStream::connect(args.address).await.unwrap();
            let mut con = NetMsgConnection::new(stream);
            con.init();
            let msg = NetMsg::Hello {
                localhost: "localhost".to_string(),
            };

            con.write(msg).await;
            if let Some(msg) = con.read().await {
                // println!("{:?}", msg);
                copy_remote_file(&mut con, "demo.bin".into(), "test.bin".into()).await;
            }
        }
    }
}

async fn server_loop(socket: TcpStream) {
    println!("entering server loop.");
    let mut con = NetMsgConnection::new(socket);
    let mut file: Option<_> = None;
    while let Some(msg) = con.read().await {
        let res = match msg {
            NetMsg::Hello { localhost } => NetMsg::Message {
                text: format!("hello {}", localhost),
            },
            NetMsg::OpenRequest { name } => match OpenOptions::new().read(true).open(name) {
                Ok(f) => {
                    file = Some(f);
                    NetMsg::OpenResponse { handle: 1 }
                }
                Err(e) => NetMsg::Message {
                    text: format!("error opening: {}", e),
                },
            },
            NetMsg::ReadRequest { handle, start, len } => match &file {
                Some(f) => {
                    // println!("reading {}-{}", start, len);

                    let mut buf = Vec::with_capacity(len);
                    buf.resize(len, 0u8);
                    #[cfg(windows)]
                    match f.seek_read(&mut buf, start as u64) {
                        Ok(n) => NetMsg::ReadResponse {
                            data: buf[..n].to_vec(),
                        },
                        Err(e) => NetMsg::Message {
                            text: format!("error reading, {}", e),
                        },
                    }
                    #[cfg(unix)]
                    match f.read_at(&mut buf, start as u64) {
                        Ok(n) => NetMsg::ReadResponse {
                            data: buf[..n].to_vec(),
                        },
                        Err(e) => NetMsg::Message {
                            text: format!("error reading, {}", e),
                        },
                    }
                }
                None => NetMsg::Message {
                    text: format!("error reading, file not open."),
                },
            },
            _ => NetMsg::Message {
                text: format!("server doesn't understand '{:?}'", msg),
            },
        };
        con.write(res).await;
    }
    println!("client disconnected.");
}
