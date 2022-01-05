use std::{io, net::SocketAddr, path::PathBuf, sync::Arc};

use async_stream::stream;
use hyper::{
    server::accept,
    service::{make_service_fn, service_fn},
    Server,
};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::error;

use crate::service::file_service;

pub async fn server(
    listen: SocketAddr,
    mut crypto: rustls::ServerConfig,
    root: Arc<Option<PathBuf>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Build TLS configuration.
    let tls_cfg = {
        // Configure ALPN to accept HTTP/2, HTTP/1.1 in that order.
        crypto.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Arc::new(crypto)
    };

    let port = listen.port();

    // Create a TCP listener via tokio.
    let tcp = TcpListener::bind(&listen).await?;
    let tls_acceptor = TlsAcceptor::from(tls_cfg);
    // Prepare a long-running future stream to accept and serve clients.
    let incoming_tls_stream = stream! {
        loop {
            let (socket, _) = tcp.accept().await?;
            match tls_acceptor.accept(socket).await {
                Ok(s) => yield Ok::<_, io::Error>(s),
                Err(e) => {
                    // Errors could be handled here, instead of server aborting.
                    // Ok(None)
                    error!("TLS Error: {:?}", e);
                    continue;
                }
            }
        }
    };
    let acceptor = accept::from_stream(incoming_tls_stream);
    let service = make_service_fn(move |_| {
        let root = root.clone();
        async move {
            Ok::<_, io::Error>(service_fn(move |r| {
                let root = root.clone();
                file_service(r, port, root)
            }))
        }
    });
    let server = Server::builder(acceptor).serve(service);

    // Run the future, keep going until an error occurs.
    println!("Starting to serve on https://{}.", listen);
    server.await?;
    Ok(())
}
