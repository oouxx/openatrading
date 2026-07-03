use crate::cli::app::ConfigAction;
use crate::cli::config::AppConfig;
use anyhow::Result;

pub fn cmd_config(action: ConfigAction) -> Result<()> {
    let cfg = AppConfig::load()?;

    match action {
        ConfigAction::Show => {
            if let Some(p) = cfg.llm_provider() {
                println!("llm.provider = {p:?}");
            }
            if let Some(m) = cfg.llm_model() {
                println!("llm.model = {m:?}");
            }
            if let Some(ch) = cfg.social_notify_channel() {
                println!("social.notify_channel = {ch:?}");
            }
            if let Some(tg) = cfg.social_telegram() {
                if let Some(t) = &tg.bot_token {
                    println!("social.telegram.bot_token = {t:?}");
                }
                if let Some(id) = &tg.chat_id {
                    println!("social.telegram.chat_id = {id:?}");
                }
            }
            if let Some(dc) = cfg.social_discord() {
                if let Some(t) = &dc.bot_token {
                    println!("social.discord.bot_token = {t:?}");
                }
                if let Some(id) = &dc.channel_id {
                    println!("social.discord.channel_id = {id:?}");
                }
            }
        },
        ConfigAction::Llm => {
            println!("provider = {:?}", cfg.llm_provider());
            println!("model    = {:?}", cfg.llm_model());
        },
        ConfigAction::Social => {
            println!("notify_channel     = {:?}", cfg.social_notify_channel());
            if let Some(tg) = cfg.social_telegram() {
                println!("telegram.bot_token = {:?}", tg.bot_token);
                println!("telegram.chat_id   = {:?}", tg.chat_id);
            }
            if let Some(dc) = cfg.social_discord() {
                println!("discord.bot_token  = {:?}", dc.bot_token);
                println!("discord.channel_id = {:?}", dc.channel_id);
            }
        },
        ConfigAction::SetProvider { value } => {
            AppConfig::set_value("llm", "provider", &value)?;
            println!("✅ llm.provider = {value:?}");
        },
        ConfigAction::SetModel { value } => {
            AppConfig::set_value("llm", "model", &value)?;
            println!("✅ llm.model = {value:?}");
        },
        ConfigAction::SetNotifyChannel { value } => {
            AppConfig::set_value("social", "notify_channel", &value)?;
            println!("✅ social.notify_channel = {value:?}");
        },
        ConfigAction::SetTelegramToken { value } => {
            AppConfig::set_value("social.telegram", "bot_token", &value)?;
            println!("✅ social.telegram.bot_token = {value:?}");
        },
        ConfigAction::SetTelegramChatId { value } => {
            AppConfig::set_value("social.telegram", "chat_id", &value)?;
            println!("✅ social.telegram.chat_id = {value:?}");
        },
        ConfigAction::SetDiscordToken { value } => {
            AppConfig::set_value("social.discord", "bot_token", &value)?;
            println!("✅ social.discord.bot_token = {value:?}");
        },
        ConfigAction::SetDiscordChannelId { value } => {
            AppConfig::set_value("social.discord", "channel_id", &value)?;
            println!("✅ social.discord.channel_id = {value:?}");
        },
    }
    Ok(())
}
