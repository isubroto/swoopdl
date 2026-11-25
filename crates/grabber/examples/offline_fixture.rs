//! Generate a local acceptance artifact; not a production capture command.
use sdl_grabber::{
    ProjectConfig, ProjectId, ProjectSnapshot,
    crawl::{CrawlTransport, run},
    store::ProjectStore,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass an empty artifact directory")?,
    );
    let server = sdl_labserver::LabServer::start(Default::default()).await?;
    let mut config = ProjectConfig::new(
        "Offline acceptance fixture",
        server.url("/capture/index.html").parse()?,
        root.join("output"),
    );
    config.allow_private_seed = true;
    let access = sdl_engine::crawl_fetch::NetworkAccess::with_private_origin(
        config.seed.origin().ascii_serialization(),
    );
    let store = ProjectStore::open(&root.join("metadata"), &ProjectId::parse("fixture")?)?;
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let state = run(
        config.clone(),
        ProjectSnapshot::new(config),
        store.clone(),
        CrawlTransport::new(Default::default(), Arc::new(access)),
        rx,
        tx,
    )
    .await?;
    let checkpoint_started = std::time::Instant::now();
    store.checkpoint(&state)?;
    let checkpoint_ms = checkpoint_started.elapsed().as_millis();
    let raw_bodies: std::collections::BTreeSet<_> =
        state.resources.values().map(|r| &r.raw_hash).collect();
    println!(
        "status={:?}; resources={}; decoded_bytes={}; requests={}; entry={}",
        state.status,
        state.resources.len(),
        state.consumed_bytes,
        server.log().len(),
        state.config.destination.join("index.html").display()
    );
    println!(
        "unique_urls={}; unique_raw_bodies={}; checkpoint_ms={checkpoint_ms}",
        state.seen.len(),
        raw_bodies.len()
    );
    Ok(())
}
