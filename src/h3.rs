use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use bytes::Bytes;
use futures::StreamExt;
use h3::{quic::BidiStream, server::RequestStream};
use http::{Request, Response};
use hyper::body::HttpBody as _;
use tracing::{debug, error, info, trace_span, warn};

use crate::service::file_service;

static ALPN: &[u8] = b"h3";

pub async fn server(
    listen: SocketAddr,
    mut crypto: rustls::ServerConfig,
    root: Arc<Option<PathBuf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    crypto.max_early_data_size = u32::MAX;
    crypto.alpn_protocols = vec![ALPN.into()];
    let server_config = h3_quinn::quinn::ServerConfig::with_crypto(Arc::new(crypto));

    let (endpoint, mut incoming) = h3_quinn::quinn::Endpoint::server(server_config, listen.into())?;

    let port = listen.port();

    info!(
        "Listening on port {:?}",
        endpoint.local_addr().unwrap().port()
    );

    while let Some(new_conn) = incoming.next().await {
        trace_span!("New connection being attempted");

        let root = root.clone();
        tokio::spawn(async move {
            match new_conn.await {
                Ok(conn) => {
                    debug!("New connection now established");

                    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn))
                        .await
                        .unwrap();

                    while let Some((req, stream)) = h3_conn.accept().await.unwrap() {
                        let root = root.clone();
                        debug!("connection requested: {:#?}", req);

                        tokio::spawn(async move {
                            if let Err(e) = handle_request(req, stream, port, root).await {
                                error!("request failed with: {}", e);
                            }
                        });
                    }
                }
                Err(err) => {
                    warn!("connecting client failed with error: {:?}", err);
                }
            }
        });
    }
    Ok(())
}

async fn handle_request<T>(
    req: Request<()>,
    mut stream: RequestStream<T, Bytes>,
    port: u16,
    serve_root: Arc<Option<PathBuf>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: BidiStream<Bytes>,
{
    let resp = file_service(req, port, serve_root).await?;
    let (parts, mut body) = resp.into_parts();
    let resp = Response::from_parts(parts, ());

    match stream.send_response(resp).await {
        Ok(_) => {
            debug!("Response to connection successful");
        }
        Err(err) => {
            error!("Unable to send response to connection peer: {:?}", err);
        }
    }

    while let Some(data) = body.data().await {
        stream.send_data(data?).await?;
    }

    Ok(stream.finish().await?)
}
