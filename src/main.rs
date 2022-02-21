use std::sync::Arc;

use tokio::io::{self, BufReader, AsyncBufReadExt};
use tokio::sync::broadcast::{channel, Sender};
use tokio_stream::wrappers::BroadcastStream as StreamWrapper;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use actix_web::{App, HttpServer, Responder, HttpResponse};
use actix_web::web::{Data, Bytes};

use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Address to run the HTTP server on
    address: String,
}

#[actix_web::get("/")]
async fn stream(data: Data<Arc<Sender<Bytes>>>) -> impl Responder {
    // get the receiver from the Arc and turn it into an async stream
    let rx = StreamWrapper::new((*data.into_inner()).subscribe());

    // stream the data to the requester
    HttpResponse::Ok()
        .content_type("text/event_stream")
        .no_chunking(u64::MAX)
        .streaming::<_, BroadcastStreamRecvError>(rx)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // parse arguments
    let args = Args::parse();

    // create a channel, capable of handling multiple readers
    let (sender, _) = channel::<Bytes>(16);

    let tx = Arc::new(sender);
    let tx_data = Arc::clone(&tx);

    // start streaming stdin
    tokio::spawn(async move {
        let mut stdin = BufReader::new(io::stdin()).lines();

        while let Some(mut block) = stdin.next_line().await.unwrap() {
            block = block + "\n";
            tx.send(block.into()).ok();
        }
    });

    // setup the stream to the http server
    let server = HttpServer::new(move ||
            App::new()
                .app_data(Data::new(Arc::clone(&tx_data)))
                .service(stream)
        );

    // actually connect and run the web server, depending on the address
    if args.address.starts_with("unix:") {
        server.bind_uds(&args.address[5..])
    } else {
        server.bind(args.address)
    }?
    .run()
    .await
}
