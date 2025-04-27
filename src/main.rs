use futures::SinkExt;
use reqwest::header;
use sea_orm::*;
use std::{
    env::var,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{self, Instant};
use tokio_stream::StreamExt;
use tokio_tungstenite::{
    connect_async, tungstenite::client::IntoClientRequest, tungstenite::protocol::Message,
};

struct Config {
    misskey_host: String,
    misskey_token: String,
    db_path: String,
}

fn get_environment_variable(key: &str) -> String {
    var(key).unwrap_or_else(|_| panic!("Environment variable `{}` was not found!", key))
}

// TODO: 例外処理
#[tokio::main]
async fn main() {
    let config = Arc::new(Config {
        misskey_host: get_environment_variable("MISSKEY_HOST"),
        misskey_token: get_environment_variable("MISSKEY_TOKEN"),
        db_path: get_environment_variable("DB_PATH"),
    });

    // Connect to DB
    let db: DatabaseConnection = Database::connect(format!("sqlite://{}?mode=rwc", config.db_path))
        .await
        .unwrap();

    // Prepare HTTP/HTTPS Client
    let request_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .unwrap();

    // Run Sub Thread
    let reminder_handler = std::thread::spawn({
        let config = config.clone();
        let db = db.clone();
        let request_client = request_client.clone();
        || reminder(config, db, request_client)
    });

    // Connect to misskey with websocket
    let ws_request = format!(
        "wss://{}/streaming?i={}",
        config.misskey_host, config.misskey_token
    )
    .into_client_request()
    .unwrap();
    let (mut ws_socket, _) = connect_async(ws_request)
        .await
        .expect("Failed to connect Websocket!");

    // Connect to main channel
    ws_socket
        .send(Message::text(
            r#"{"type": "connect", "body": {"channel": "main", "id": "mi-reminder-main"}}"#,
        ))
        .await
        .expect("Failed to connect to main channel.");
    ws_socket.flush().await.unwrap();

    // Read WebSocketStream continuously
    // TODO: サーバー側切断への対処
    while let Some(responce) = ws_socket.next().await {
        let responce = responce.unwrap();

        if responce.is_close() {
            break;
        }
        if responce.is_empty() {
            continue;
        }

        let responce: serde_json::Value =
            serde_json::from_str(responce.to_text().unwrap_or_default()).unwrap();

        if responce["type"] == "channel" && responce["body"]["type"] == "mention" {
            tokio::task::spawn(process_note(
                config.clone(),
                responce["body"]["body"].clone(),
            ));
        }
    }

    reminder_handler.join().unwrap(); // TODO: DB観点で何らかのタイミングで正常終了させる必要がある?
    ws_socket.close(None).await.unwrap();
}

async fn process_note(config: Arc<Config>, body: serde_json::Value) {
    println!("{}", body); // TODO: 内容
                          // TODO: matchで書いているが、ifにして条件をより具体的に記述した方が良さそう
    match body["visibility"].as_str() {
        Some("public") => try_register(&config, body).await,
        Some("home") => try_register(&config, body).await,
        _ => (),
    }
}

// Register or Reject remind
async fn try_register(config: &Arc<Config>, body: serde_json::Value) {
    // TODO
    println!("remind処理対象です");
}

// Remove the remind
async fn remove_remind() {
    // TODO
}

#[tokio::main]
async fn reminder(config: Arc<Config>, db: DatabaseConnection, request_client: reqwest::Client) {
    let request_url = format!("https://{}/api/notes/create", config.misskey_host);

    // Wait until initial process timing
    let until_start = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(unixtime) => 60_000 - (unixtime.as_millis() % 60_000) as u64,
        Err(_) => 0,
    };
    time::sleep_until(Instant::now() + Duration::from_millis(until_start)).await;

    // Interval Start (Triggered at every *h*m00s)
    let mut interval = time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        // If this is called for the first time, it is completed immediately.
        interval.tick().await;
        send_remind(&config, &db, &request_client, &request_url).await;
    }
}

async fn send_remind(
    config: &Arc<Config>,
    db: &DatabaseConnection,
    request_client: &reqwest::Client,
    request_url: &String,
) {
    println!("Tick!"); // TODO: 仮
                       //TODO: fetch remind targets from DB

    //TODO: DBからの取得結果に対してforeach的な非同期処理
        let request_responce = request_client
            .post(request_url)
            .header(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/json"),
            )
            .body(format!(r#"{{"text": "test投稿", "reactionAcceptance": "likeOnly", "visibility": "home", "i": "{}"}}"#, config.misskey_token))
            .send()
            .await
            .expect("Failed to create note!");

        println!(
            "{}, {}",
            request_responce.status(),
            request_responce.text().await.unwrap()
        );
    //TODO: Update DB for next remind timing
}
