//! Socket utilities for creating listeners, async readers, writers, and dialers.
use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::{
    io::{self, AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, UdpSocket},
};
use tonic::Status;
use tracing::error;

/// create a tcp listener.
pub(crate) async fn create_tcp_listener(port: u16) -> Result<TcpListener, Status> {
    TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(map_bind_error)
}

/// create a udp socket.
pub(crate) async fn create_udp_socket(port: u16) -> Result<UdpSocket, Status> {
    UdpSocket::bind(("0.0.0.0", port))
        .await
        .map_err(map_bind_error)
}

fn map_bind_error(err: std::io::Error) -> Status {
    match err.kind() {
        std::io::ErrorKind::AddrInUse => Status::already_exists("port is already in use"),
        std::io::ErrorKind::PermissionDenied => Status::permission_denied("permission denied"),
        _ => {
            error!("failed to bind port: {}", err);
            Status::internal("failed to bind port")
        }
    }
}

/// Async reader for a udp connection.
pub(crate) struct AsyncUdpSocket<'a> {
    socket: &'a UdpSocket,
}

impl<'a> AsyncUdpSocket<'a> {
    pub(crate) fn new(socket: &'a UdpSocket) -> Self {
        Self { socket }
    }
}

impl<'a> AsyncRead for AsyncUdpSocket<'a> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut().socket.poll_recv_from(cx, buf) {
            Poll::Ready(Ok(_addr)) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<'a> AsyncWrite for AsyncUdpSocket<'a> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::result::Result<usize, std::io::Error>> {
        self.get_mut().socket.poll_send(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>> {
        Poll::Ready(Ok(())) // No-op for UDP
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>> {
        Poll::Ready(Ok(())) // No-op for UDP
    }
}

/// Dialer for connecting to a endpoint to get a async reader and a async writer.
#[derive(Debug)]
pub(crate) struct Dialer {
    dial: DialFn,
    addr: SocketAddr,
}

impl Dialer {
    /// Create a new dialer.
    pub(crate) fn new(dial: DialFn, addr: SocketAddr) -> Self {
        Self { dial, addr }
    }

    /// Dial the endpoint.
    ///
    /// # Returns
    ///
    /// A future that resolves to a async reader and a async writer.
    pub(crate) fn dial(&self) -> Pin<Box<dyn std::future::Future<Output = DialResult> + Send>> {
        (self.dial)(self.addr)
    }

    /// Expose the address of the dialer.
    pub(crate) fn addr(&self) -> SocketAddr {
        self.addr
    }
}

/// Result of dialing a endpoint.
pub(crate) type DialResult = Result<
    (
        Box<dyn AsyncRead + Unpin + Send>,
        Box<dyn AsyncWrite + Unpin + Send>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
>;

/// Function for dialing a endpoint to get a async reader and a async writer.
pub(crate) type DialFn =
    fn(SocketAddr) -> Pin<Box<dyn std::future::Future<Output = DialResult> + Send>>;

/// Dial a tcp endpoint.
pub(crate) async fn dial_tcp(local_endpoint: SocketAddr) -> DialResult {
    let local_conn = TcpStream::connect(local_endpoint).await?;
    let (r, w) = local_conn.into_split();
    Ok((Box::new(r), Box::new(w)))
}

/// Dial a udp endpoint.
pub(crate) async fn dial_udp(local_endpoint: SocketAddr) -> DialResult {
    let local_addr: SocketAddr = if local_endpoint.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()?;
    let socket = UdpSocket::bind(local_addr).await?;
    socket.connect(local_endpoint).await?;
    let socket = Box::leak(Box::new(socket));
    Ok((
        Box::new(AsyncUdpSocket::new(socket)),
        Box::new(AsyncUdpSocket::new(socket)),
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::net::TcpListener as StdTcpListener;

    #[tokio::test]
    async fn test_tcp_listener_and_dialer() {
        let port = free_port().unwrap();
        let listener = create_tcp_listener(port).await.unwrap();

        let dialer = Dialer::new(
            |addr| Box::pin(dial_tcp(addr)),
            listener.local_addr().unwrap(),
        );
        dialer.dial().await.unwrap();
    }

    #[tokio::test]
    async fn test_udp_socket_and_dialer() {
        let port = free_port().unwrap();
        let socket = create_udp_socket(port).await.unwrap();

        let dialer = Dialer::new(
            |addr| Box::pin(dial_udp(addr)),
            socket.local_addr().unwrap(),
        );
        dialer.dial().await.unwrap();
    }

    /// free_port returns a free port number for testing.
    fn free_port() -> std::io::Result<u16> {
        let listener = StdTcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        Ok(port)
    }
}
