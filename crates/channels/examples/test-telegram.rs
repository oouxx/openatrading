//! Telegram 适配器快速测试。
//!
//! 用法:
//!   TELEGRAM_BOT_TOKEN=xxx TELEGRAM_CHAT_ID=xxx cargo run --example test-telegram
//!
//! 启动后会监听 60 秒，向指定 chat 发送的消息会打印到终端。

use openatrading_channels::adapter::{launch_adapters, MessageAdapter};
use openatrading_channels::telegram::TelegramAdapter;
use std::sync::Arc;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let token = std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN");
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").expect("TELEGRAM_CHAT_ID");

    let adapter: Arc<dyn MessageAdapter> = Arc::new(TelegramAdapter::new(token, chat_id));
    let (mut rx, adapters, _handles) = launch_adapters(vec![adapter], 64);

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

            // 回复
            if let Some(adapter) = adapters.first() {
                adapter.reply(&event, "收到 ✅").await.ok();
            }
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
            println!("⏳ 超时，未收到消息");
        }
    }
}
