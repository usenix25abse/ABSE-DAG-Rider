use std::net::SocketAddr;
use std::str::FromStr;
use anyhow::{Context, Result};
use bytes::BufMut as _;
use bytes::BytesMut;
use clap::{App, AppSettings, crate_name, crate_version};
use env_logger::Env;
use futures::sink::SinkExt as _;
use log::{info};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .args_from_usage("<ADDR> 'The network address of the node where to send txs'")
        .args_from_usage("--TRANSACTION_COUNT=[COUNT] 'The number of transactions to send, default 10000'")
        .args_from_usage("--TX_SIZE=[SIZE] 'The size of each transaction, default 128'")
        .setting(AppSettings::ArgRequiredElseHelp)
        .get_matches();

    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let target = matches
        .value_of("ADDR")
        .unwrap()
        .parse::<SocketAddr>()
        .context("Invalid socket address format")?;

    let transaction_count = u64::from_str(matches.value_of("TRANSACTION_COUNT").unwrap_or("10000"))?;
    let tx_size = usize::from_str(matches.value_of("TX_SIZE").unwrap_or("128"))?;

    info!("Node address: {}", target);
    info!("Transaction count: {}", transaction_count);
    info!("Transaction size: {}", tx_size);

    let client = Client {
        target,
        transaction_count,
        tx_size,
    };

    // Start the benchmark.
    client.send().await.context("Failed to submit transactions")
}

struct Client {
    target: SocketAddr,
    transaction_count: u64,
    tx_size: usize,
}

impl Client {
    pub async fn send(&self) -> Result<()> {
        let TRANSACTION_COUNT: u64 = self.transaction_count.clone();
        let TX_SIZE: usize = self.tx_size.clone();

        let stream = TcpStream::connect(self.target)
            .await
            .context(format!("failed to connect to {}", self.target))?;

        let mut tx = BytesMut::with_capacity(TX_SIZE);
        let mut transport = Framed::new(stream, LengthDelimitedCodec::new());

        info!("Start sending transactions");

        for c in 0..TRANSACTION_COUNT {
            info!("Sending sample transaction {}", c);

            tx.put_u8(0u8); // Sample txs start with 0.
            tx.put_u64(c); // This counter identifies the tx.
            // tx.resize(TX_SIZE, 0u8);
            let bytes = tx.split().freeze();

            transport.send(bytes).await?;
        }

        Ok(())
    }
}
