pub mod cls;
pub mod cninfo;
pub mod eastmoney;
pub mod jiuyangongshe;
pub mod sina;
pub mod taoguba;
pub mod utils;
pub mod xuangubao;
use cls::ClsClient;

use cninfo::CninfoClient;
use jiuyangongshe::JiuYanSession;
use xuangubao::XuanguBaoClient;

use crate::eastmoney::EastmoneyClient;
#[derive(Clone)]
pub struct Client {
    pub xuangubao: XuanguBaoClient,
    pub jiuyan: JiuYanSession,
    pub cls: ClsClient,
    pub eastmoney: EastmoneyClient,
    pub cninfo: CninfoClient,
}

impl Client {
    pub fn new() -> Self {
        Self {
            xuangubao: XuanguBaoClient::new(),
            jiuyan: JiuYanSession::new(),
            cls: ClsClient::new(),
            eastmoney: EastmoneyClient::new(),
            cninfo: CninfoClient::new(),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
