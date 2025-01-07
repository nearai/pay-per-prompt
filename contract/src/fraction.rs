use near_sdk::json_types::U128;
use near_sdk::{near, NearToken};

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Fraction {
    num: U128,
    den: U128,
}

impl Default for Fraction {
    fn default() -> Self {
        Self {
            num: U128(0),
            den: U128(1),
        }
    }
}

impl Fraction {
    pub fn percent(percent: u8) -> Self {
        assert!(percent <= 100);
        Self {
            num: U128(percent.into()),
            den: U128(100),
        }
    }

    pub fn is_less_than_one(&self) -> bool {
        self.num.0 < self.den.0
    }

    pub fn is_zero(&self) -> bool {
        self.num.0 == 0
    }

    pub fn mul(&self, other: u128) -> u128 {
        self.num.0 * other / self.den.0
    }

    pub fn mul_balance(&self, balance: NearToken) -> NearToken {
        NearToken::from_yoctonear(self.mul(balance.as_yoctonear()))
    }
}
