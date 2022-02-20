use tokio::io::{self, BufReader, AsyncBufReadExt};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

use futures_util::StreamExt;

use actix_web::{App, HttpServer, Responder, HttpResponse};
use actix_web::web::Data;

#[actix_web::get("/")]
async fn stream(data: Data<watch::Receiver<Option<String>>>) -> impl Responder {
    // get the receiver from the Arc and turn it into an async stream
    let rx = WatchStream::new((*data.into_inner()).clone());

    // stream the data to the requester
    HttpResponse::Ok()
        .content_type("text/event_stream")
        .no_chunking(u64::MAX)
        .streaming(rx.map(|data| match data {
            // map the data to valid input/output for the connection
            Some(data) => Ok(data.into()),
            _ => Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Error")),
        }))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // create a channel, capable of handling multiple readers
    let (tx, rx) = watch::channel::<Option<String>>(Some(String::from("\n")));

    tokio::spawn(async move {
        let mut stdin = BufReader::new(io::stdin()).lines();

        // continually read lines from standard input
        while let Some(mut line) = stdin.next_line().await.unwrap() {
            line = line + "\n";

            // send the line into the receiver
            tx.send(Some(line.into())).unwrap()
        }
    });

    // start the http server
    HttpServer::new(move ||
            App::new()
                .app_data(Data::new(rx.clone()))
                .service(stream)
        )
        .bind("localhost:8000")?
        .run()
        .await
}
