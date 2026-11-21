//! `mock-hermes [--addr 127.0.0.1:9120] [--speed 1.0]`: run the mock Hermes gateway.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut addr = "127.0.0.1:9120".to_string();
    let mut speed = 1.0f64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--addr" => addr = args.next().unwrap_or(addr),
            "--speed" => speed = args.next().and_then(|s| s.parse().ok()).unwrap_or(speed),
            "--help" | "-h" => {
                println!("mock-hermes [--addr HOST:PORT] [--speed FACTOR]");
                return Ok(());
            }
            _ => {}
        }
    }
    let (local, _mock, handle) = mock_hermes::serve(&addr, speed).await?;
    println!("mock-hermes listening on http://{local}");
    handle.await?;
    Ok(())
}
