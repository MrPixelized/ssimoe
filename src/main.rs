use std::sync::Arc;

use tokio::io::{self, BufReader, AsyncBufReadExt};
use tokio::sync::broadcast::{channel, Sender};
use tokio_stream::wrappers::BroadcastStream as StreamWrapper;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use actix_web::{App, HttpServer, Responder, HttpResponse};
use actix_web::web::{Data, Bytes};

use clap::Parser;

#[derive(Parser, Clone)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Address to run the HTTP server on
    address: String,

    /// Content type
    #[clap(short, long, default_value = "text/event_stream")]
    content_type: String,
}

struct CommonData {
    args: Arc<Args>,
    sender: Arc<Sender<Bytes>>,
}

#[actix_web::get("/")]
async fn stream(data: Data<CommonData>) -> impl Responder {
    // get the receiver from the Arc and turn it into an async stream
    let rx = StreamWrapper::new(data.sender.subscribe());

    // stream the data to the requester
    HttpResponse::Ok()
        // use the stored content type
        .content_type(data.args.content_type.clone())
        .no_chunking(u64::MAX)
        .streaming::<_, BroadcastStreamRecvError>(rx)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // parse arguments
    let args = Args::parse();
    let data_args = Arc::new(args.clone());

    // create a channel, capable of handling multiple readers
    let (sender, _) = channel::<Bytes>(16);

    // make sure one end can be sent to connecting tasks, and one is kept here
    // for writing
    let sender = Arc::new(sender);
    let data_sender = Arc::clone(&sender);

    // start streaming stdin
    tokio::spawn(async move {
        let mut stdin = BufReader::new(io::stdin()).lines();

        while let Some(mut block) = stdin.next_line().await.unwrap() {
            block = block + "\n";
            sender.send(block.into()).ok();
        }
    });

    // setup the stream to the http server
    let server = HttpServer::new(move ||
            App::new()
                .app_data(Data::new(CommonData {
                    args: Arc::clone(&data_args),
                    sender: Arc::clone(&data_sender)
                }))
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
