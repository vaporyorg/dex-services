use ethcontract::Address;
use std::collections::HashMap;

/// Maps a user and a token id to the balance the user has of this token.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountState(pub HashMap<(Address, u16), u128>);

impl AccountState {
    pub fn read_balance(&self, token_id: u16, account_id: Address) -> u128 {
        self.0.get(&(account_id, token_id)).cloned().unwrap_or(0)
    }

    pub fn user_token_pairs(&self) -> impl Iterator<Item = (Address, u16)> + '_ {
        self.0.iter().map(|(&pair, _)| pair)
    }
}

#[cfg(test)]
mod test_util {
    use super::*;
    use crate::models::Order;

    impl AccountState {
        pub fn new(balances: Vec<u128>, num_tokens: u16) -> Self {
            assert_eq!(
                balances.len() % (num_tokens as usize),
                0,
                "Elements in balance vector needs to be a multiple of num_tokens"
            );
            let balances = balances
                .chunks(num_tokens as usize)
                .enumerate()
                .flat_map(|(account, token_balances)| {
                    token_balances
                        .iter()
                        .enumerate()
                        .map(move |(token, balance)| {
                            let key = (Address::from_low_u64_be(account as u64), token as u16);
                            (key, *balance)
                        })
                })
                .collect();
            AccountState(balances)
        }

        pub fn with_balance_for(orders: &[Order]) -> Self {
            let mut account_state = AccountState::default();
            for order in orders {
                account_state.increase_balance(
                    order.account_id,
                    order.sell_token,
                    order.sell_amount,
                );
            }
            account_state
        }

        pub fn increase_balance(&mut self, account_id: Address, token_id: u16, amount: u128) {
            *self.0.entry((account_id, token_id)).or_default() += amount;
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn test_cannot_create_with_bad_balance_length() {
        AccountState::new(vec![100, 200], 30);
    }
}
