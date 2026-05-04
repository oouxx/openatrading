pub mod jiuyangongshe;
pub mod taoguba;
pub mod xuangubao;

use jiuyangongshe::JiuYanSession;
use taoguba::TgbClient;
use xuangubao::XuanguBaoClient;

pub struct Client {
    pub xuangubao: XuanguBaoClient,
    pub jiuyan: JiuYanSession,
    pub taoguba: TgbClient,
}

impl Client {
    pub fn new() -> Self {
        Self {
            xuangubao: XuanguBaoClient::new(),
            jiuyan: JiuYanSession::new(),
            taoguba: TgbClient::new(),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
