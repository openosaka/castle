use bytes::Bytes;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// IdBridge is id with [`Bridge`].
pub struct IdDataSenderBridge {
    pub id: Bytes,
    pub inner: DataSenderBridge,
}

/// DataSenderBridge is used for sending data to data server.
///
/// the data server creates DataSenderBridge then sends it to control server,
/// then the control server uses the `chan` to send data to data server.
pub struct DataSenderBridge {
    /// BridgeChan is used for sending data to data server.
    chan: DataSender,
    /// when the server receives [`tunneld_protocol::pb::traffic_to_server::Action::Close`] action from [`tunneld_protocol::pb::tunnel_service_server::TunnelService::data`] streaming,
    /// the server will cancel the bridge.
    cancel: CancellationToken,
}

impl DataSenderBridge {
    /// new creates a new DataSenderBridge.
    pub fn new(chan: DataSender, cancel: CancellationToken) -> Self {
        Self { chan, cancel }
    }

    /// send sends data to data server.
    pub async fn send_data(&self, data: Vec<u8>) -> Result<(), mpsc::error::SendError<BridgeData>> {
        self.chan.send(BridgeData::Data(data)).await
    }

    /// send_sender sends sender to data server.
    pub async fn send_sender(
        &self,
        sender: mpsc::Sender<Vec<u8>>,
    ) -> Result<(), mpsc::error::SendError<BridgeData>> {
        self.chan.send(BridgeData::Sender(sender)).await
    }

    /// close closes the bridge.
    pub fn close(&self) {
        self.cancel.cancel();
    }
}

/// DataSender is the sender holden by control server to send data to data server.
///
/// this channel has two purposes:
/// 1. when the server receives [`tunneld_protocol::pb::traffic_to_server::Action::Start`] action from [`tunneld_protocol::pb::tunnel_service_server::TunnelService::data`] streaming,
/// the server will send [`BridgeChanData::DataSender`] through this channel,
/// 2. when the server receives [`tunneld_protocol::pb::traffic_to_server::Action::Sending`],
/// the server will send [`BridgeChanData::Data`] through this channel.
pub type DataSender = mpsc::Sender<BridgeData>;

/// BridgeData is the data of the `BridgeChan`.
pub enum BridgeData {
    // Sender is used for sending [`BridgeData::Data`].
    Sender(mpsc::Sender<Vec<u8>>),
    Data(Vec<u8>),
}
