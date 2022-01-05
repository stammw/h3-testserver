use std::{io, path::PathBuf, sync::Arc};

use async_stream::stream;
use bytes::{Bytes, BytesMut};
use http::{Request, Response, StatusCode};
use tokio::{fs::File, io::AsyncReadExt};
use tracing::debug;

pub async fn file_service<T>(
    req: Request<T>,
    port: u16,
    serve_root: Arc<Option<PathBuf>>,
) -> Result<Response<hyper::Body>, io::Error> {
    let (status, body) = match serve_root.as_deref() {
        None => (StatusCode::IM_A_TEAPOT, hyper::Body::empty()),
        Some(_) if req.uri().path().contains("..") => (StatusCode::NOT_FOUND, hyper::Body::empty()),
        Some(root) => {
            let to_serve = root.join(req.uri().path().strip_prefix("/").unwrap_or(""));
            match File::open(&to_serve).await {
                Ok(mut file) => (
                    StatusCode::OK,
                    hyper::Body::wrap_stream(stream! {
                        loop {
                            let mut buf = BytesMut::with_capacity(4096 * 1024);
                            let read = file.read_buf(&mut buf).await?;
                            if read == 0 {
                                break;
                            }
                            yield Ok::<Bytes, std::io::Error>(buf.freeze());
                        }
                    }),
                ),
                Err(e) => {
                    debug!("failed to open: \"{}\": {}", to_serve.to_string_lossy(), e);
                    (StatusCode::NOT_FOUND, hyper::Body::empty())
                }
            }
        }
    };

    Ok(http::Response::builder()
        .status(status)
        .header("Alt-Svc", format!("h3=\":{port}\"", port = port))
        .body(body)
        .unwrap())
}
