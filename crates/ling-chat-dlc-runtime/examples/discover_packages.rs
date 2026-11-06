//! Live read-only release discovery; downloads manifests, never package archives.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let source = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: discover_packages OWNER/REPOSITORY"))?;
    let catalog = ling_chat_dlc_runtime::games::discovery::discover(&source).await?;
    println!("{}", serde_json::to_string_pretty(&catalog)?);
    Ok(())
}
