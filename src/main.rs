use futures::future;
use rustls::{Certificate, PrivateKey};
use std::{io::Cursor, net::SocketAddr, path::PathBuf, sync::Arc};
use structopt::StructOpt;
use tokio::{fs::File, io::AsyncReadExt};
use tracing::{error, info, trace};

mod h2;
mod h3;
mod service;

// Configs for two server modes
// selfsigned mode will generate it's own local certificate
// certs mode will require a path to 2-3 files(cert, key, ca)
#[derive(StructOpt, Debug)]
#[structopt(name = "server")]
struct Opt {
    #[structopt(name = "dir", short)]
    pub serve_root: Option<PathBuf>,

    #[structopt(default_value = "[::]:4433")]
    pub addrs: Vec<SocketAddr>,

    #[structopt(flatten)]
    pub certs: Certs,
}

#[derive(StructOpt, Debug)]
pub struct Certs {
    #[structopt(long)]
    pub cert: Option<PathBuf>,

    #[structopt(long)]
    pub key: Option<PathBuf>,

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .with_writer(std::io::stderr)
        .init();

    let opt = Opt::from_args();
    trace!("{:#?}", opt);

    let serve_root = Arc::new(if let Some(serve_root) = opt.serve_root {
        if serve_root.is_dir() && serve_root.read_dir().is_err() {
            let err = format!(
                "{}: is not a readable directory",
                serve_root.to_string_lossy()
            );
            error!("{}", err);
            return Err(err.into());
        } else {
            info!("serving {}", serve_root.display());
            Some(serve_root)
        }
    } else {
        None
    });

    let crypto = load_crypto(opt.certs).await?;

    let mut listeners = opt
        .addrs
        .iter()
        .map(|a| {
            let a = a.clone();
            let crypto = crypto.clone();
            let root = serve_root.clone();
            tokio::spawn(async move {
                if let Err(e) = h3::server(a, crypto, root).await {
                    error!("server failed with: {}", e);
                }
            })
        })
        .collect::<Vec<_>>();

    let mut listeners_tcp = opt
        .addrs
        .into_iter()
        .map(|a| {
            let crypto = crypto.clone();
            let root = serve_root.clone();
            tokio::spawn(async move {
                if let Err(e) = h2::server(a, crypto, root).await {
                    error!("server failed with: {}", e);
                }
            })
        })
        .collect::<Vec<_>>();

    listeners.append(&mut listeners_tcp);

    future::join_all(listeners).await;

    Ok(())
}

pub fn build_certs() -> (Vec<Certificate>, PrivateKey) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = PrivateKey(cert.serialize_private_key_der());
    let cert = Certificate(cert.serialize_der().unwrap());
    (vec![cert], key)
}

async fn load_crypto(opt: Certs) -> Result<rustls::ServerConfig, Box<dyn std::error::Error>> {
    let (cert, key) = match (opt.cert, opt.key) {
        (None, None) => build_certs(),
        (Some(cert_path), Some(ref key_path)) => {
            let mut cert_file = File::open(cert_path).await?;
            let mut key_file = File::open(key_path).await?;
            let mut cert_buf = Vec::new();
            let mut key_buf = Vec::new();
            cert_file.read_to_end(&mut cert_buf).await?;
            key_file.read_to_end(&mut key_buf).await?;

            let certs = rustls_pemfile::certs(&mut Cursor::new(cert_buf))?
                .into_iter()
                .map(rustls::Certificate)
                .collect();
            let key = rustls_pemfile::pkcs8_private_keys(&mut Cursor::new(key_buf))?
                .into_iter()
                .map(rustls::PrivateKey)
                .collect::<Vec<_>>();
            if key.is_empty() {
                return Err(format!("no keys found in {}", key_path.display()).into());
            }

            (certs, key[0].clone())
        }
        (_, _) => return Err("cert and key args are mutually dependant".into()),
    };

    let crypto = rustls::ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(cert, key)?;

    Ok(crypto)
}
