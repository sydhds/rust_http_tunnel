use core::fmt::Debug;
use std::net::SocketAddr;
use std::env;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream, UnixListener};
use tokio::signal;
use tokio::time::timeout;
// Tls
use tokio_rustls::TlsAcceptor;

// traits
// use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt}; // for read_buf() / write()
use tokio_util::codec::{Decoder, Encoder}; // for encode() / decode()
use futures::StreamExt; // for next()

mod codec;
use crate::codec::{HttpCodec, TunnelResult};
mod dns;
mod tls;
use crate::tls::{load_certs, load_keys};

use crate::dns::{DnsResolver, SimpleDnsResolver}; // for decode()

// Easy error handling with async code
type AResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const PROXY_INITIAL_RESPONSE_SIZE: usize = 64;
const PROXY_CONNECT_TARGET_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_millis(200);


async fn tunnel_relay<R, W>(mut reader: R, mut writer: W, addr: SocketAddr) -> AResult<()>
    where R: AsyncRead + Send + Unpin + 'static,
          W: AsyncWrite + Send + Unpin + 'static
{
    let mut codec = HttpCodec {};
    let mut response_buffer = bytes::BytesMut::with_capacity(PROXY_INITIAL_RESPONSE_SIZE);

    // connect to destination then write ok response then relay data in both direction
    // match TcpStream::connect(&addr[..]).await {
    match timeout(PROXY_CONNECT_TARGET_TIMEOUT,
                  TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => {

            // Note: no need to use FrameWrite here
            // write response to proxy
            codec.encode(TunnelResult::Ok, &mut response_buffer);
            writer.write_buf(&mut response_buffer).await?;

            stream.writable().await?;
            let (mut stream_reader, mut stream_writer) = stream.into_split();
            let r1 = tokio::spawn(async move {
                // from proxy client to dest writer
                tokio::io::copy(&mut reader, &mut stream_writer).await
            });

            let r2 = tokio::spawn(async move {
                // from dest reader to proxy writer
                tokio::io::copy(&mut stream_reader, &mut writer).await
            });

        }
        Ok(Err(e)) => {
            // connect error
            println!("Could not connect to {}: {}", addr, e);
            codec.encode(TunnelResult::Timeout, &mut response_buffer);
            writer.write_buf(&mut response_buffer).await?;
        }
        Err(e) => {
            // timeout
            println!("Timeout while trying to connect to {}: {}", addr, e);
            codec.encode(TunnelResult::BadRequest, &mut response_buffer);
            writer.write_buf(&mut response_buffer).await?;
        },
    }

    Ok(())
}


async fn tunnel_stream<R, W, D>(mut reader: R, mut writer: W, mut resolver: D) -> AResult<()>
    where R: AsyncRead + Send + Unpin + Debug + 'static,
          W: AsyncWrite + Send + Unpin + 'static,
          D: DnsResolver
{
    let mut codec = HttpCodec {};
    // let mut buffer = bytes::BytesMut::new(); // TODO: capacity?
    let mut url = String::new();
    let mut n = 0;

    let mut fr = tokio_util::codec::FramedRead::new(reader, codec);
    // println!("fr: {:?}", fr);

    // TODO: timeout
    if let Ok(url_) = fr.next().await.ok_or("Cannot read frame")? {
        // println!("{}", url_);
        let addr = resolver.resolve(&url_).await?;
        let reader = fr.into_inner(); // get back reader
        tokio::spawn(tunnel_relay(reader, writer, addr));
    }
    Ok(())
}

async fn tunnel() -> AResult<()> {

    // Skip args[0] (cmd line string) and only take first
    let arg: Vec<String> = env::args().skip(1).take(3).collect();
    let resolver = SimpleDnsResolver::new();

    let empty_str = String::new();
    let (addr, cert, key, enable_tls) = match arg.len() {
        0 => panic!("Please provide a host:port like 127.0.0.1:7070"), // TODO: proper error
        1 => (&arg[0], &empty_str, &empty_str, false),
        3 => (&arg[0], &arg[1], &arg[2], true),
        _ => panic!("..."), // TODO: proper error
    };

    println!("addr: {}", cert);
    println!("Enable tls: {}", enable_tls);

    // TODO: timeout
    match enable_tls {
        true => {

            let certs = load_certs(cert)?;
            let mut keys = load_keys(key)?;

            let config = rustls::ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                // .with_single_cert(certs, keys.remove(0))
                .with_single_cert(certs, keys.pop().ok_or("Unable to read key")?)
                .map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
                })?;
            let acceptor = TlsAcceptor::from(Arc::new(config));

            let listener = TcpListener::bind(&addr[..]).await?;
            println!("[Tcp/Tls] Listening on {}", addr);
            loop {
                let (socket, _addr) = listener.accept().await?;
                let acceptor_ = acceptor.clone();
                let mut stream = acceptor.accept(socket).await?;
                let (mut reader, mut writer) = tokio::io::split(stream);
                let resolver_ = resolver.clone();

                tokio::spawn(async move {
                    if let Err(e) = tunnel_stream(reader, writer, resolver_).await {
                        println!("[Tcp/Tls] Tunnel stream error: {}", e);
                    }
                });
            }
        },
        false => {
            let listener = TcpListener::bind(&addr[..]).await?;
            println!("[Tcp] Listening on {}", addr);
            loop {
                let (socket, _addr) = listener.accept().await?;
                socket.writable().await?;
                let (mut reader, mut writer) = socket.into_split();
                let resolver_ = resolver.clone();

                tokio::spawn(async move {
                    if let Err(e) = tunnel_stream(reader, writer, resolver_).await {
                        println!("[Tcp] Tunnel stream error: {}", e);
                    }
                });
            }
        }
    }
}

async fn app_main() -> AResult<()> {
    println!("Starting http tunnel...");
    tokio::select! {
        tunnel_result = tunnel() => {
            if tunnel_result.is_err() {
                println!("Unable to start tunnel: {:?}", tunnel_result);
            }
        },
        _ = signal::ctrl_c() => { println!("\nReceived [Ctrl-C]..."); },
    };
    Ok(())
}

fn main() {

    // init the tokio async runtime - default is a multi threaded runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    // app_main func is our main entry point
    // TODO: handle Result and return a integer (like a regular linux cmd)
    rt.block_on(app_main());

}
