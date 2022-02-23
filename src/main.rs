use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{self, BufReader, AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::broadcast::{channel, Sender};
use tokio_stream::wrappers::BroadcastStream as StreamWrapper;
use tokio_stream::StreamExt;

use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Address to run the HTTP server on
    address: String,
}

async fn handle_incoming(mut connection: TcpStream,
                         mut stdin: StreamWrapper<String>,
                         _args: Arc<Args>) -> io::Result<()> {
    while let Some(Ok(block)) = stdin.next().await {
        connection.write_all(block.as_bytes()).await?;
    }

    Err(std::io::ErrorKind::BrokenPipe.into())
}

async fn stream_stdin_to(sender: Sender<String>) {
    let mut stdin = BufReader::new(io::stdin()).lines();

    while let Some(mut block) = stdin.next_line().await.unwrap() {
        block = block + "\n";
        sender.send(block.into()).ok();
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // parse arguments
    let args = Arc::new(Args::parse());

    // create a channel, capable of handling multiple readers
    let (sender, _) = channel::<String>(16);

    // spawn a task to stream stdin to this channel
    tokio::spawn(stream_stdin_to(sender.clone()));

    // start receiving and handling requests
    let listener = TcpListener::bind(args.address.clone()).await?;

    loop {
        // listen for an incoming request
        let (connection, _) = match listener.accept().await {
            Ok(x) => x,
            Err(_) => continue,
        };

        // setup a new receiving channel
        let stream = StreamWrapper::new(sender.subscribe());

        // spawn a new task to forward it to the incoming connection
        tokio::spawn(handle_incoming(connection, stream, Arc::clone(&args)));
    }
}
