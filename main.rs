use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server, Uri};
use hyper::http::StatusCode;
use std::convert::Infallible;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::net::SocketAddr;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let backends = vec![
        "http://localhost:4000",
        "http://localhost:4001",
    ];

    let shared_backends = Arc::new(backends);
    let counter = Arc::new(AtomicUsize::new(0));

    let make_svc = make_service_fn(move |_| {
        let backends = shared_backends.clone();
        let counter = counter.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let backends = backends.clone();
                let counter = counter.clone();
                async move {
                    match forward_request(req, backends, counter).await {
                        Ok(response) => Ok(response),
                        Err(e) => {
                            eprintln!("Request failed: {}", e);
                            Ok::<_, Infallible>(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Body::from("Internal Server Error"))
                                .unwrap())
                        }
                    }
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}

async fn forward_request(
    req: Request<Body>,
    backends: Arc<Vec<&str>>,
    counter: Arc<AtomicUsize>,
) -> Result<Response<Body>, hyper::Error> {
    let backend = {
        let index = counter.fetch_add(1, Ordering::SeqCst) % backends.len();
        backends[index].to_string()
    };

    let client = Client::new();

    let uri_string = format!("{}{}", backend, req.uri().path_and_query().map(|x| x.as_str()).unwrap_or(""));
    let uri: Uri = uri_string.parse().unwrap();

    let mut new_req_builder = Request::builder()
        .method(req.method())
        .uri(uri);

    for (key, value) in req.headers().iter() {
        new_req_builder = new_req_builder.header(key, value);
    }

    let new_req = new_req_builder
        .body(req.into_body())
        .unwrap();

    client.request(new_req).await
}
