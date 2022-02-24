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

    /// String that delimits buffered blocks of standard input
    #[clap(long, short)]
    block_delimiter: Option<String>,
}

async fn handle_incoming(mut connection: TcpStream,
                         mut stdin: StreamWrapper<String>,
                         _args: Arc<Args>) -> io::Result<()> {
    while let Some(Ok(block)) = stdin.next().await {
        connection.write_all(block.as_bytes()).await?;
    }

    Err(std::io::ErrorKind::BrokenPipe.into())
}

async fn stream_stdin_to(sender: Sender<String>, args: Arc<Args>) {
    let mut stdin = BufReader::new(io::stdin()).lines();
    let delim = args.block_delimiter.clone();

    let mut block = String::new();

    while let Some(ref line) = stdin.next_line().await.unwrap() {
        // If there is no delimiter or it has not been reached,
        // add this line to the block
        if delim != Some(line.to_string()) {
            block += line.as_ref();
            block += "\n";
        }

        // If there is no delimiter or this line is the delimter,
        // send the block onward and clear it
        if delim.is_none() || Some(line.to_string()) == delim {
            sender.send(block.clone()).ok();
            block.clear();
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // parse arguments
    let args = Arc::new(Args::parse());

    // create a channel, capable of handling multiple readers
    let (sender, _) = channel::<String>(16);

    // spawn a task to stream stdin to this channel
    tokio::spawn(stream_stdin_to(sender.clone(), Arc::clone(&args)));

    // start receiving and handling requests
    let listener = TcpListener::bind(&args.address).await?;

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
