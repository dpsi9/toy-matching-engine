use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::Message;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tracing::debug;

use shared::models::Fill;

pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    tx: web::Data<broadcast::Sender<Fill>>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;
    let mut fills_stream = BroadcastStream::new(tx.subscribe());

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                incoming = msg_stream.next() => {
                    match incoming {
                        Some(Ok(Message::Ping(bytes))) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(Message::Close(reason))) => {
                            let _ = session.close(reason).await;
                            break;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(err)) => {
                            debug!(error = %err, "websocket incoming stream error");
                            break;
                        }
                        None => break,
                    }
                }

                fill_item = fills_stream.next() => {
                    match fill_item {
                        Some(Ok(fill)) => {
                            match serde_json::to_string(&fill) {
                                Ok(payload) => {
                                    if session.text(payload).await.is_err() {
                                        break;
                                    }
                                }
                                Err(err) => {
                                    debug!(error = %err, "failed to serialize fill");
                                }
                            }
                        }
                        Some(Err(_lagged)) => {
                            // Drop lagged notifications and continue with latest.
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(response)
}

