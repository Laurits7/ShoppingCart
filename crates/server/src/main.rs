#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `shoppingcart healthcheck`: for Docker HEALTHCHECK in an image without curl.
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(if healthcheck() { 0 } else { 1 });
    }
    shoppingcart::run().await
}

fn healthcheck() -> bool {
    use std::io::{Read, Write};
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(shoppingcart::config::DEFAULT_PORT);
    let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(3)));
    if s.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 16];
    s.read(&mut buf).is_ok_and(|n| {
        buf[..n].starts_with(b"HTTP/1.1 200") || buf[..n].starts_with(b"HTTP/1.0 200")
    })
}
