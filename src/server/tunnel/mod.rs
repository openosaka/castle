use crate::{
    bridge::{self, DataSenderBridge, IdDataSenderBridge},
    event,
};
use anyhow::Context as _;
use bytes::Bytes;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tonic::Status;
use uuid::Uuid;

use super::port::{Available, PortManager};

pub(crate) mod http;
pub(crate) mod tcp;
pub(crate) mod udp;

pub(crate) struct BridgeResult {
    pub data_sender: mpsc::Sender<Vec<u8>>,
    pub data_receiver: mpsc::Receiver<bridge::BridgeData>,
    pub client_cancel_receiver: CancellationToken,
    /// the caller should cancel this token when it finishes the transfer.
    pub remove_bridge_sender: CancellationToken,
}

/// init_data_sender_bridge creates a bridge between the control server and data server.
///
/// In this function, it has been sent the bridge to the control server,
/// and wait to receive the first message which is [`crate::bridge::BridgeData::Sender`] from the control server.
pub(crate) async fn init_data_sender_bridge(
    user_incoming_chan: mpsc::Sender<event::UserIncoming>,
) -> anyhow::Result<BridgeResult> {
    let bridge_id = Bytes::from(Uuid::new_v4().to_string());
    let (bridge_chan, mut bridge_chan_receiver) = mpsc::channel(1024);

    let client_cancel = CancellationToken::new();
    let client_cancel_receiver = client_cancel.clone();

    let event = IdDataSenderBridge {
        id: bridge_id.clone(),
        inner: DataSenderBridge::new(bridge_chan.clone(), client_cancel),
    };
    user_incoming_chan
        .send(event::UserIncoming::Add(event))
        .await
        .context("send user incoming event, this operation should not fail")
        .unwrap();

    let remove_bridge_sender = CancellationToken::new();
    let remove_bridge_receiver = remove_bridge_sender.clone();
    let bridge_id_clone = bridge_id.clone();
    tokio::spawn(async move {
        remove_bridge_receiver.cancelled().await;
        user_incoming_chan
            .send(event::UserIncoming::Remove(bridge_id_clone))
            .await
            .context("notify server to remove connection channel")
            .unwrap();
    });

    let data_sender = tokio::select! {
        data_sender = bridge_chan_receiver.recv() => {
            match data_sender {
                Some(bridge::BridgeData::Sender(sender)) => sender,
                _ => panic!("we expect to receive DataSender from data_channel_rx at the first time."),
            }
        }
        _ = client_cancel_receiver.cancelled() => {
            remove_bridge_sender.cancel();
            return Err(anyhow::anyhow!("client cancelled"))
        }
    };

    Ok(BridgeResult {
        data_sender,
        data_receiver: bridge_chan_receiver,
        client_cancel_receiver,
        remove_bridge_sender,
    })
}

pub(crate) trait SocketCreator {
    type Output;

    async fn create_socket(port: u16) -> anyhow::Result<Self::Output, Status>;
}

pub(crate) async fn create_socket<T: SocketCreator>(
    port: u16,
    port_manager: &mut PortManager,
) -> anyhow::Result<(Available, T::Output), Status> {
    if port > 0 {
        let socket = T::create_socket(port).await?;
        Ok((port.into(), socket))
    } else {
        // refer: https://github.com/ekzhang/bore/blob/v0.5.1/src/server.rs#L88
        // todo: a better way to find a free port
        loop {
            let port: Available = match port_manager.get() {
                None => {
                    return Err(Status::resource_exhausted("no available port"));
                }
                Some(port) => port,
            };
            let result = T::create_socket(*port).await;
            if result.is_err() {
                port_manager.remove(*port);
                continue;
            }
            return Ok((port, result.unwrap()));
        }
    }
}
