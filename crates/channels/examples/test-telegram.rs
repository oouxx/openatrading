//! Telegram 适配器快速测试。
//!
//! 用法:
//!   TELEGRAM_BOT_TOKEN=xxx TELEGRAM_CHAT_ID=xxx cargo run --example test-telegram
//!
//! 启动后会监听 60 秒，向指定 chat 发送的消息会打印到终端。

use openatrading_channels::adapter::MessageAdapter;
use openatrading_channels::telegram::TelegramAdapter;
use tokio::sync::mpsc;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let token = std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN");
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").expect("TELEGRAM_CHAT_ID");

    let adapter = TelegramAdapter::new(token, chat_id);
    let (tx, mut rx) = mpsc::channel(64);

    let handle = tokio::spawn(async move {
        if let Err(e) = Box::new(adapter).start(tx).await {
            eprintln!("adapter error: {e}");
        }
    });

    println!("监听中… 向 Telegram bot 发一条消息（超时 60 秒）");

    tokio::select! {
        Some(event) = rx.recv() => {
            println!("✅ 收到消息:");
            println!("   platform:   {:?}", event.platform);
            println!("   user_id:    {}", event.user_id);
            println!("   user_name:  {:?}", event.user_name);
            println!("   channel_id: {}", event.channel_id);
            println!("   message_id: {}", event.message_id);
            println!("   text:       {}", event.text);
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
            println!("⏳ 超时，未收到消息");
        }
    }

    handle.abort();
}
