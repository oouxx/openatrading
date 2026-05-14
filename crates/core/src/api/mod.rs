pub mod cls;
pub mod jiuyangongshe;
pub mod taoguba;
pub mod xuangubao;
use cls::ClsClient;
use jiuyangongshe::JiuYanSession;
use taoguba::TgbClient;
use xuangubao::XuanguBaoClient;
#[derive(Clone)]
pub struct Client {
    pub xuangubao: XuanguBaoClient,
    pub jiuyan: JiuYanSession,
    pub taoguba: TgbClient,
    pub cls: ClsClient,
}

impl Client {
    pub fn new() -> Self {
        Self {
            xuangubao: XuanguBaoClient::new(),
            jiuyan: JiuYanSession::new(),
            taoguba: TgbClient::new(),
            cls: ClsClient::new(),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
