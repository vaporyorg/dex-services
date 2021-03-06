use super::*;
use crate::{
    contracts::{
        stablex_contract::{batch_exchange, StableXContract},
        Web3,
    },
    models::{AccountState, Order},
    orderbook::StableXOrderBookReading,
};
use anyhow::{anyhow, bail, ensure, Result};
use block_timestamp_reading::{BlockTimestampReading, CachedBlockTimestampReader};
use ethcontract::{contract::Event, errors::ExecutionError, H256};
use futures::{
    channel::oneshot,
    future::FutureExt,
    pin_mut, select_biased,
    stream::{Stream, StreamExt as _},
};
use orderbook::Orderbook;
use std::collections::HashSet;
use std::future::Future;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::{process, thread, time::Duration};

/// An event based orderbook that automatically updates itself with new events from the contract.
#[derive(Debug)]
pub struct UpdatingOrderbook {
    orderbook: Arc<Mutex<Orderbook>>,
    // Indicates whether the background thread has caught up with past events at which point the
    // orderbook is ready to be read.
    orderbook_ready: Arc<AtomicBool>,
    // When this struct is dropped this sender will be dropped which makes the updater thread stop.
    _exit_tx: oneshot::Sender<()>,
}

impl UpdatingOrderbook {
    pub fn new(contract: &dyn StableXContract, web3: Web3) -> Self {
        let orderbook = Arc::new(Mutex::new(Orderbook::default()));
        let orderbook_clone = orderbook.clone();
        let orderbook_ready = Arc::new(AtomicBool::new(false));
        let orderbook_ready_clone = orderbook_ready.clone();
        let (exit_tx, exit_rx) = oneshot::channel();
        // Create stream first to make sure we do not miss any events between it and past events.
        let stream = contract.stream_events();
        let past_events = contract.past_events();

        std::thread::spawn(move || {
            let result = futures::executor::block_on(update_with_events_forever(
                orderbook_clone,
                orderbook_ready_clone,
                CachedBlockTimestampReader::new(web3),
                exit_rx,
                past_events,
                stream,
            ));
            if let Err(err) = result {
                log::error!("event based orderbook failed: {:?}", err);
                // TODO: implement a retry mechanism
                // For now we error the program so force a restart of the whole driver because
                // without a retry we would be stuck with an outdated orderbook forever.
                // Sleep for one second, so that we have time to flush the logs.
                thread::sleep(Duration::from_secs(1));
                process::exit(1);
            }
        });

        Self {
            orderbook,
            orderbook_ready,
            _exit_tx: exit_tx,
        }
    }
}

impl StableXOrderBookReading for UpdatingOrderbook {
    fn get_auction_data(&self, batch_id_to_solve: U256) -> Result<(AccountState, Vec<Order>)> {
        ensure!(
            self.orderbook_ready.load(Ordering::SeqCst),
            "orderbook not yet ready"
        );
        self.orderbook
            .lock()
            .map_err(|err| anyhow!("poison error: {}", err))?
            .get_auction_data(batch_id_to_solve)
    }
}

/// Update the orderbook with events from the stream forever or until exit_indicator is dropped.
///
/// Returns Ok when exit_indicator is dropped.
/// Returns Err if the stream ends.
async fn update_with_events_forever(
    orderbook: Arc<Mutex<Orderbook>>,
    orderbook_ready: Arc<AtomicBool>,
    mut block_timestamp_reader: CachedBlockTimestampReader<Web3>,
    exit_indicator: oneshot::Receiver<()>,
    past_events: impl Future<Output = Result<Vec<Event<batch_exchange::Event>>, ExecutionError>>,
    stream: impl Stream<Item = Result<Event<batch_exchange::Event>, ExecutionError>>,
) -> Result<()> {
    // `select!` requires the futures to be fused...
    let exit_indicator = exit_indicator.fuse();
    let past_events = past_events.fuse();
    let stream = stream.fuse();
    // ...and pinned.
    pin_mut!(exit_indicator);
    pin_mut!(past_events);
    pin_mut!(stream);

    log::info!("Starting event based orderbook updating.");

    loop {
        // We select over everything together instead of for example the past events first then the
        // stream to ensure that the stream gets polled at least once which it needs in order to
        // create the corresponding filter on the node.
        select_biased! {
            _ = exit_indicator => return Ok(()),
            event = stream.next() => {
                log::info!("Received new event.");
                let event = event.ok_or(anyhow!("stream ended"))??;
                handle_event(&orderbook, &mut block_timestamp_reader, event).await?;
            },
            past_events = past_events => {
                let past_events = past_events?;
                log::info!("Received {} past events.", past_events.len());
                let block_hashes = past_events.iter().map(|event| {
                    let metadata = event.meta.as_ref().ok_or(anyhow!("event without metadata: {:?}", event))?;
                    Ok(metadata.block_hash)
                }).collect::<Result<HashSet<H256>>>()?;
                block_timestamp_reader.prepare_cache(block_hashes).await?;
                for event in past_events {
                    handle_event(&orderbook, &mut block_timestamp_reader, event).await?;
                }
                log::info!("Finished applying past events");
                orderbook_ready.store(true, Ordering::SeqCst);
            },
        };
    }
}

/// Apply a single event to the orderbook.
async fn handle_event(
    orderbook: &Mutex<Orderbook>,
    block_timestamp_reader: &mut impl BlockTimestampReading,
    event: Event<batch_exchange::Event>,
) -> Result<()> {
    match event {
        Event {
            data,
            meta: Some(meta),
        } => {
            let block_timestamp = block_timestamp_reader
                .block_timestamp(meta.block_hash)
                .await?;
            orderbook
                .lock()
                .map_err(|e| anyhow!("poison error: {}", e))?
                .handle_event_data(
                    data,
                    meta.block_number,
                    meta.log_index,
                    meta.block_hash,
                    block_timestamp,
                );
            Ok(())
        }
        Event { meta: None, .. } => bail!("event without metadata"),
    }
}
