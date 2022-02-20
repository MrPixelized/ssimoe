use tokio::io::{self, BufReader, AsyncBufReadExt};
use tokio::sync::watch::{channel, Receiver};
use tokio_stream::wrappers::WatchStream as StreamWrapper;

use futures_util::StreamExt;

use actix_web::{App, HttpServer, Responder, HttpResponse};
use actix_web::web::Data;

use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Address to run the HTTP server on
    address: String,
}

#[actix_web::get("/")]
async fn stream(data: Data<Receiver<String>>) -> impl Responder {
    // get the receiver from the Arc and turn it into an async stream
    let rx = StreamWrapper::new((*data.into_inner()).clone());

    // stream the data to the requester
    HttpResponse::Ok()
        .content_type("text/event_stream")
        .no_chunking(u64::MAX)
        .streaming::<_, std::io::Error>(rx.map(|data| Ok(data.into())))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // parse arguments
    let args = Args::parse();

    // create a channel, capable of handling multiple readers
    let (tx, rx) = channel::<String>(String::from("\n"));

    tokio::spawn(async move {
        let mut stdin = BufReader::new(io::stdin()).lines();

        // continually read lines from standard input
        while let Some(mut block) = stdin.next_line().await.unwrap() {
            block = block + "\n";

            // send the line into the receiver
            tx.send(block).ok();
        }
    });

    // setup the stream to the http server
    let server = HttpServer::new(move ||
            App::new()
                .app_data(Data::new(rx.clone()))
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
