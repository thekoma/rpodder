//! Feed fetches ignore a proxy set in the environment while non-public
//! addresses are refused: the proxy would resolve host names itself, out of
//! reach of the check on resolved addresses.
//!
//! A test binary of its own, because it sets process-wide variables.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn a_proxy_from_the_environment_is_not_used() {
    // A "proxy" that answers every request with a feed.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let body = "<rss version=\"2.0\"><channel><title>Proxied</title></channel></rss>";
            let head = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len());
            let _ = sock.write_all(format!("{head}{body}").as_bytes()).await;
        }
    });
    // SAFETY: the only test in this binary; set before any client is built.
    unsafe { std::env::set_var("HTTP_PROXY", format!("http://{proxy}")) };

    rpodder_feed::allow_private_addresses(false);
    let fetcher = rpodder_feed::FeedFetcher::with_limits(Duration::from_secs(5), 1);
    let result = fetcher
        .fetch("http://feed.example.test/feed.xml", None, None)
        .await;
    assert!(result.is_err());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
