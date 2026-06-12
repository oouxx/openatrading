use crate::roles::pnl::INITIAL_CAPITAL;

const MAX_SINGLE_POSITION_RATIO: f64 = 0.3;
const MAX_TOTAL_POSITION_RATIO: f64 = 0.9;
const MIN_SHARES: i64 = 100;

/// A股模拟撮合引擎，负责滑点计算、费用核算、手数取整和仓位约束校验。
/// 纯计算，无副作用，不持有任何可变状态。
pub struct PaperBroker {
    /// 模拟滑点，单位 bps（基点），1 bps = 0.01%
    pub slippage_bps: f64,
    /// 印花税率（卖出时收取），A 股目前为 0.001 = 0.1%
    pub stamp_duty_rate: f64,
    /// 佣金率（双向收取），如 0.0003 = 万三
    pub commission_rate: f64,
    /// 最低佣金（元）
    pub min_commission: f64,
}

impl Default for PaperBroker {
    fn default() -> Self {
        Self {
            slippage_bps: 10.0,
            stamp_duty_rate: 0.001,
            commission_rate: 0.0003,
            min_commission: 5.0,
        }
    }
}

impl PaperBroker {
    /// 滑点后买入成交价
    pub fn buy_price(&self, last_price: f64) -> f64 {
        last_price * (1.0 + self.slippage_bps / 10000.0)
    }

    /// 滑点后卖出成交价
    pub fn sell_price(&self, last_price: f64) -> f64 {
        last_price * (1.0 - self.slippage_bps / 10000.0)
    }

    /// A股手数取整（100 股 = 1 手，向下取整）
    pub fn round_shares(shares: i64) -> i64 {
        (shares / MIN_SHARES) * MIN_SHARES
    }

    /// 计算买入费用，返回 (成交金额, 佣金)
    pub fn buy_cost(&self, price: f64, shares: i64) -> (f64, f64) {
        let total = price * shares as f64;
        let commission = (total * self.commission_rate).max(self.min_commission);
        (total, commission)
    }

    /// 计算卖出费用，返回 (成交金额, 佣金, 印花税)
    pub fn sell_cost(&self, price: f64, shares: i64) -> (f64, f64, f64) {
        let total = price * shares as f64;
        let commission = (total * self.commission_rate).max(self.min_commission);
        let stamp_duty = total * self.stamp_duty_rate;
        (total, commission, stamp_duty)
    }

    /// 校验买入：手数 >= 100、手数为 100 倍数、含佣金后不超单票上限和总仓位上限
    pub fn validate_buy(
        &self,
        shares: i64,
        exec_price: f64,
        same_symbol_value: f64,
        total_position_value: f64,
    ) -> Result<(), String> {
        if shares < MIN_SHARES {
            return Err(format!(
                "shares {} is less than minimum {}",
                shares, MIN_SHARES
            ));
        }
        if shares % MIN_SHARES != 0 {
            return Err(format!(
                "shares {} is not a multiple of {}",
                shares, MIN_SHARES
            ));
        }

        let (total, commission) = self.buy_cost(exec_price, shares);
        let cost = total + commission;

        if cost + same_symbol_value > INITIAL_CAPITAL * MAX_SINGLE_POSITION_RATIO {
            return Err(format!(
                "single stock position {:.0} + new {:.0} exceeds max {:.0} ({}% of capital)",
                same_symbol_value,
                cost,
                INITIAL_CAPITAL * MAX_SINGLE_POSITION_RATIO,
                (MAX_SINGLE_POSITION_RATIO * 100.0) as i32
            ));
        }

        if total_position_value + cost > INITIAL_CAPITAL * MAX_TOTAL_POSITION_RATIO {
            return Err(format!(
                "total position value {:.0} + new {:.0} exceeds max {:.0} ({}% of capital)",
                total_position_value,
                cost,
                INITIAL_CAPITAL * MAX_TOTAL_POSITION_RATIO,
                (MAX_TOTAL_POSITION_RATIO * 100.0) as i32
            ));
        }

        Ok(())
    }

    /// 校验卖出：持仓存在且可卖
    pub fn validate_sell(symbol: &str, shares: i64) -> Result<(), String> {
        if shares == 0 {
            return Err(format!("no open position found for {}", symbol));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buy_price_with_slippage() {
        let broker = PaperBroker::default();
        let price = broker.buy_price(10.0);
        assert!(price > 10.0);
        assert!((price - 10.01).abs() < 0.001); // 10 bps
    }

    #[test]
    fn test_sell_price_with_slippage() {
        let broker = PaperBroker::default();
        let price = broker.sell_price(10.0);
        assert!(price < 10.0);
    }

    #[test]
    fn test_round_shares() {
        assert_eq!(PaperBroker::round_shares(250), 200);
        assert_eq!(PaperBroker::round_shares(99), 0);
        assert_eq!(PaperBroker::round_shares(100), 100);
    }

    #[test]
    fn test_buy_cost_min_commission() {
        let broker = PaperBroker::default();
        // 买 100 股 @ 10 元 = 1000 元，万三佣金 = 0.3 元，不足最低 5 元
        let (total, commission) = broker.buy_cost(10.0, 100);
        assert_eq!(total, 1000.0);
        assert_eq!(commission, 5.0);
    }

    #[test]
    fn test_sell_cost() {
        let broker = PaperBroker::default();
        let (total, commission, stamp_duty) = broker.sell_cost(10.0, 1000);
        assert_eq!(total, 10000.0);
        assert_eq!(commission, 5.0); // 万三=3 元，取最低 5 元
        assert_eq!(stamp_duty, 10.0); // 0.1%
    }

    #[test]
    fn test_validate_buy_rejects_odd_lot() {
        let broker = PaperBroker::default();
        assert!(broker.validate_buy(150, 10.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn test_validate_buy_ok() {
        let broker = PaperBroker::default();
        assert!(broker.validate_buy(100, 10.0, 0.0, 0.0).is_ok());
    }
}
