#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    gatewayd::run(gatewayd::RuntimeOptions::community()).await
}
