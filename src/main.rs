use futures::SinkExt;
use std::{
    env::var,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{self, Instant};
use tokio_stream::StreamExt;
use tokio_tungstenite::{
    connect_async, tungstenite::client::IntoClientRequest, tungstenite::protocol::Message,
};

// TODO: 例外処理
#[tokio::main]
async fn main() {
    // run sub thread
    let reminder_handler = std::thread::spawn(reminder);

    // connect to misskey with websocket
    let ws_request = format!(
        "wss://{}/streaming?i={}",
        var("MISSKEY_HOST").expect("Environment variable `MISSKEY_HOST` was not found!"),
        var("MISSKEY_TOKEN").expect("Environment variable `MISSKEY_TOKEN` was not found!")
    )
    .into_client_request()
    .unwrap();
    let (mut ws_socket, _) = connect_async(ws_request)
        .await
        .expect("Failed to connect Websocket!");

    // connect to main channel
    ws_socket
        .send(Message::text(
            r#"{"type": "connect", "body": {"channel": "main", "id": "mi-reminder-main"}}"#,
        ))
        .await
        .expect("Failed to connect to main channel.");
    ws_socket.flush().await.unwrap();

    // TODO: サーバー側切断への対処
    let mut count = 0; // TODO: 仮
    while let Some(message) = ws_socket.next().await {
        let message = message.unwrap();
        println!("{}", message); // TODO: 仮
                                 // tokio::task::spawn(async {});
        count += 1;
        if message.is_close() || count > 3 {
            break;
        }
    }

    reminder_handler.join().unwrap(); // TODO: DB観点で何らかのタイミングで正常終了させる必要がありそう
    ws_socket.close(None).await.unwrap();
}

#[tokio::main]
async fn reminder() {
    // connect to db
    // TODO

    // wait until initial process timing
    let until_start = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(unixtime) => 60_000 - (unixtime.as_millis() % 60_000) as u64,
        Err(_) => 0,
    };
    time::sleep_until(Instant::now() + Duration::from_millis(until_start)).await;

    // interval start (triggered at every *h*m00s)
    let mut interval = time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        println!("Tick!"); // TODO: 仮
    }
}
