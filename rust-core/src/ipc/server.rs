use crate::audio::engine::EngineCommand;
use crate::audio::player::Player;
use crate::ipc::protocol::{Message, Response, Notification};
use crate::ipc::handlers::MessageHandler;
use crate::utils::error::{AudioError, Result};
use tokio_tungstenite::{accept_hdr_async, tungstenite::handshake::server::{Request as WsRequest, Response as WsResponse}};
use tokio::net::{TcpListener, TcpStream};
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use tokio::sync::Mutex; 
use crossbeam_channel::Sender;
use tracing::{info, debug, error, warn};
use std::collections::HashMap;

pub struct WebSocketServer {
    address: String,
    #[allow(dead_code)]
    command_sender: Sender<EngineCommand>,
    #[allow(dead_code)]
    player: Arc<Player>,
    handler: Arc<MessageHandler>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
}

struct Client {
    #[allow(dead_code)]
    id: String,
    sender: futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, tokio_tungstenite::tungstenite::Message>,
}

impl WebSocketServer {
    pub fn new(address: &str, command_sender: Sender<EngineCommand>, player: Arc<Player>) -> Result<Self> {
        let handler = Arc::new(MessageHandler::new(command_sender.clone(), player.clone()));

        Ok(Self {
            address: address.to_string(),
            command_sender,
            player,
            handler,
            clients: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.address).await.map_err(|e| {
            AudioError::IoError(format!("Failed to bind to {}: {}", self.address, e))
        })?;

        info!("WebSocket server listening on {}", self.address);

        while let Ok((stream, addr)) = listener.accept().await {
            let client_id = addr.to_string();
            debug!("New connection from {}", client_id);

            let handler = Arc::clone(&self.handler);
            let clients = Arc::clone(&self.clients);

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, client_id.clone(), handler, clients).await {
                    error!("Connection error for {}: {}", client_id, e);
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        client_id: String,
        handler: Arc<MessageHandler>,
        clients: Arc<Mutex<HashMap<String, Client>>>,
    ) -> Result<()> {
        let callback = |req: &WsRequest, response: WsResponse| {
            info!("WebSocket handshake from {:?}", req);
            Ok(response)
        };

        let ws_stream = accept_hdr_async(stream, callback).await?;
        let (write, mut read) = ws_stream.split();

        let client = Client {
            id: client_id.clone(),
            sender: write,
        };

        clients.lock().await.insert(client_id.clone(), client);
        info!("Client {} connected", client_id);

        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(msg) => {
                    if let Err(e) = Self::handle_message(msg, &client_id, &handler, &clients).await {
                        error!("Error handling message from {}: {}", client_id, e);
                    }
                }
                Err(e) => {
                    error!("WebSocket error for {}: {}", client_id, e);
                    break;
                }
            }
        }

        clients.lock().await.remove(&client_id);
        info!("Client {} disconnected", client_id);

        Ok(())
    }

    async fn handle_message(
        msg: tokio_tungstenite::tungstenite::Message,
        client_id: &str,
        handler: &Arc<MessageHandler>,
        clients: &Arc<Mutex<HashMap<String, Client>>>,
    ) -> Result<()> {
        match msg {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                debug!("Received message from {}: {}", client_id, text);

                match Message::from_json(&text) {
                    Ok(Message::Request(request)) => {
                        let response = handler.handle_request(request);
                        let response_msg = Message::Response(response).to_json()?;

                        if let Some(client) = clients.lock().await.get_mut(client_id) {
                            if let Err(e) = client.sender.send(tokio_tungstenite::tungstenite::Message::Text(response_msg)).await {
                                error!("Failed to send response to {}: {}", client_id, e);
                            }
                        }
                    }
                    Ok(_) => {
                        warn!("Received non-request message from {}", client_id);
                    }
                    Err(e) => {
                        error!("Failed to parse message from {}: {}", client_id, e);
                        let error_response = Response::error(None, format!("Invalid message: {}", e));
                        let error_msg = Message::Response(error_response).to_json()?;

                        if let Some(client) = clients.lock().await.get_mut(client_id) {
                            let _ = client.sender.send(tokio_tungstenite::tungstenite::Message::Text(error_msg)).await;
                        }
                    }
                }
            }
            tokio_tungstenite::tungstenite::Message::Close(_) => {
                debug!("Client {} requested close", client_id);
            }
            tokio_tungstenite::tungstenite::Message::Ping(data) => {
                debug!("Received ping from {}", client_id);
                if let Some(client) = clients.lock().await.get_mut(client_id) {
                    let _ = client.sender.send(tokio_tungstenite::tungstenite::Message::Pong(data)).await;
                }
            }
            tokio_tungstenite::tungstenite::Message::Pong(_) => {
                debug!("Received pong from {}", client_id);
            }
            _ => {
                warn!("Received unsupported message type from {}", client_id);
            }
        }

        Ok(())
    }

    pub fn broadcast_notification(&self, _notification: Notification) {
    }
}
