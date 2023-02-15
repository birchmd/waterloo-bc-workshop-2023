use block_downloader::BlockDownloader;
use chunk_downloader::ChunkDownloader;
use receipt_handler::ReceiptHandler;
use std::str::FromStr;
use tokio::task::JoinError;

mod block_downloader;
mod chunk_downloader;
mod config;
mod manager;
mod receipt_handler;
mod types;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO: allow reading from file.
    let config = config::Config::default();

    let log_level = tracing::Level::from_str(&config.log_level)?;
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let (manager_sender, manager_receiver) = tokio::sync::mpsc::channel(500);
    let (block_downloader, block_downloader_sender) =
        BlockDownloader::new(&config, manager_sender.clone(), 0).await?;
    let (chunk_downloader_tasks, chunk_downloader_channels) = {
        let mut chunk_downloader_tasks = Vec::with_capacity(config.num_chunk_downloaders.into());
        let mut chunk_downloader_channels = Vec::with_capacity(config.num_chunk_downloaders.into());
        for id in 0..config.num_chunk_downloaders {
            let (task, channel) = ChunkDownloader::new(&config, manager_sender.clone(), id.into());
            chunk_downloader_tasks.push(task.start());
            chunk_downloader_channels.push(channel);
        }
        (chunk_downloader_tasks, chunk_downloader_channels)
    };
    let (receipt_handler, receipt_channel) =
        ReceiptHandler::new(&config, manager_sender, 0).await?;

    let block_downloader_task = block_downloader.start();
    let receipt_handler_task = receipt_handler.start();
    let manager_task = manager::Manager::new(
        manager_receiver,
        block_downloader_sender,
        chunk_downloader_channels,
        receipt_channel,
    )
    .start();

    log_error("BlockDownloader", block_downloader_task.await);
    log_error("ReceiptHandler", receipt_handler_task.await);
    for task in chunk_downloader_tasks {
        log_error("ChunkDownloader", task.await);
    }
    log_error("Manager", manager_task.await);

    Ok(())
}

fn log_error(name: &str, outcome: Result<anyhow::Result<()>, JoinError>) {
    match outcome {
        Ok(Ok(_)) => (),
        Ok(Err(e)) => {
            tracing::error!("{} task exited with error: {:?}", name, e);
        }
        Err(e) => {
            tracing::error!("{} task exited with error: {:?}", name, e);
        }
    }
}
