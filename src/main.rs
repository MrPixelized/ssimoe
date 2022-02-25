use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{self, BufReader, AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::broadcast::{channel, Sender};
use tokio::sync::RwLock;
use tokio_stream::wrappers::BroadcastStream as StreamWrapper;
use tokio_stream::{self as stream, StreamExt};

use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Address to run the HTTP server on
    address: String,

    /// String that delimits buffered blocks of standard input
    #[clap(long, short)]
    delimiter: Option<String>,

    /// How many blocks of buffered input to send
    #[clap(long, short, default_value_t=0)]
    buffered_blocks: usize,
}

async fn handle_incoming(mut connection: TcpStream,
                         stdin: StreamWrapper<String>,
                         buffer: Arc<RwLock<Vec<String>>>,
                         args: Arc<Args>) -> io::Result<()> {
    // take the last "args.buffered_blocks" blocks from the buffer
    let buffer = buffer.read().await;
    let buffered_blocks = if buffer.len() < args.buffered_blocks {
        buffer.len()
    } else {
        args.buffered_blocks
    };
    let prior_blocks = buffer[buffer.len() - buffered_blocks..].to_vec();

    // release the read lock on the buffer
    drop(buffer);

    // prepend them to the standard input stream
    let buffered_blocks = stream::iter(prior_blocks).map(|block| Ok(block));
    let mut stdin = buffered_blocks.chain(stdin);

    // write OK status for HTTP protocol
    connection.write_all("HTTP/1.1 200 OK\n".as_bytes()).await?;
    connection.write_all("\n".as_bytes()).await?;

    // iterate over the items of standard input and write them to the client
    while let Some(Ok(block)) = stdin.next().await {
        connection.write_all(block.as_bytes()).await?;
    }

    Err(std::io::ErrorKind::BrokenPipe.into())
}

async fn stream_stdin_to(sender: Sender<String>,
                         buffer: Arc<RwLock<Vec<String>>>,
                         args: Arc<Args>) {
    let mut stdin = BufReader::new(io::stdin()).lines();
    let delim = args.delimiter.clone();

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
            // stream and buffer this new block
            sender.send(block.clone()).ok();

            let mut buffer = buffer.write().await;
            buffer.push(block.clone());

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

    // create a vec for buffered blocks of standard input
    let buffer = Arc::new(RwLock::new(Vec::new()));

    // spawn a task to stream stdin to this channel
    tokio::spawn(stream_stdin_to(sender.clone(), buffer.clone(), Arc::clone(&args)));

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
        tokio::spawn(
            handle_incoming(connection, stream, buffer.clone(), args.clone()));
    }
}
