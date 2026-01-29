use std::collections::HashMap;

use crate::{Account, Transaction, TransactionKind, WidthdrawAction};

pub struct Payments {
    pub accounts: Vec<Account>,
    pub actions: HashMap<u32, WidthdrawAction>,
}

impl Default for Payments {
    fn default() -> Self {
        Payments {
            accounts: vec![Account::default(); u16::MAX as usize + 1],
            actions: Default::default(),
        }
    }
}

impl Payments {
    pub fn process_transaction(&mut self, transaction: Transaction) {
        let account = self.get_account_mut(transaction.cid);
        if account.locked {
            return;
        }

        account.has_activity = true;
        match transaction.kind {
            // Processing deposits
            TransactionKind::Deposit { amount } => {
                account.total += amount;
            }

            // Processing withdrawals
            TransactionKind::Withdrawal { amount } => {
                if account.get_available() >= amount {
                    account.total -= amount;
                    self.actions.insert(
                        transaction.tid,
                        WidthdrawAction {
                            cid: transaction.cid,
                            amount,
                            disputed: false,
                        },
                    );
                }
            }

            // Processing dispute situations
            TransactionKind::Dispute | TransactionKind::Resolve | TransactionKind::Chargeback => {
                let Some(action) = self.actions.get_mut(&transaction.tid) else {
                    return;
                };

                // Checking if that transaction belonged to the client
                if action.cid != transaction.cid {
                    return;
                }

                let amount = action.amount;

                match transaction.kind {
                    TransactionKind::Dispute => {
                        // Can only dispute a transaction that not being disputed already
                        if action.disputed {
                            return;
                        }
                        action.disputed = true;
                        let account = self.get_account_mut(transaction.cid);
                        account.total += amount;
                        account.held += amount;
                    }
                    TransactionKind::Resolve => {
                        // Can only resolve/chargeback the transaction that being disputed before
                        if !action.disputed {
                            return;
                        }
                        action.disputed = false;
                        let account = self.get_account_mut(transaction.cid);
                        account.held -= amount;
                    }
                    TransactionKind::Chargeback => {
                        // Can only resolve/chargeback the transaction that being disputed before
                        if !action.disputed {
                            return;
                        }
                        action.disputed = false;
                        let account = self.get_account_mut(transaction.cid);
                        account.held -= amount;
                        account.total -= amount;
                        account.locked = true;
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    // SAFETY: we are preinitializing the whole list of accounts at start, so indexing
    // like this will always succeed
    fn get_account_mut(&mut self, cid: u16) -> &mut Account {
        unsafe { self.accounts.get_unchecked_mut(cid as usize) }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rust_decimal_macros::dec;

    fn get_active_accounts(payments: &Payments) -> Vec<(u16, Account)> {
        payments
            .accounts
            .clone()
            .iter()
            .enumerate()
            .map(|(index, account)| (index as u16, *account))
            .filter(|(_, account)| account.has_activity)
            .collect()
    }

    #[test]
    fn test_deposit() {
        let mut payments = Payments::default();
        let transactions = vec![
            Transaction {
                cid: 0,
                tid: 0,
                kind: TransactionKind::Deposit { amount: dec!(10.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Deposit { amount: dec!(20.0) },
            },
        ];

        for transaction in transactions {
            payments.process_transaction(transaction);
        }

        let active_clients = get_active_accounts(&payments);
        assert_eq!(
            active_clients,
            vec![(
                0,
                Account {
                    total: dec!(30),
                    held: dec!(0),
                    locked: false,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_withdraw() {
        let mut payments = Payments::default();
        let transactions = vec![
            Transaction {
                cid: 0,
                tid: 0,
                kind: TransactionKind::Deposit { amount: dec!(10.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Deposit { amount: dec!(20.0) },
            },
            Transaction {
                cid: 0,
                tid: 2,
                kind: TransactionKind::Withdrawal { amount: dec!(30.0) },
            },
        ];

        for transaction in transactions {
            payments.process_transaction(transaction);
        }

        let active_clients = get_active_accounts(&payments);
        assert_eq!(
            active_clients,
            vec![(
                0,
                Account {
                    total: dec!(0),
                    held: dec!(0),
                    locked: false,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_withdraw_not_enough_funds() {
        let mut payments = Payments::default();
        let transactions = vec![
            Transaction {
                cid: 0,
                tid: 0,
                kind: TransactionKind::Deposit { amount: dec!(10.0) },
            },
            Transaction {
                cid: 0,
                tid: 2,
                kind: TransactionKind::Withdrawal { amount: dec!(30.0) },
            },
        ];

        for transaction in transactions {
            payments.process_transaction(transaction);
        }

        let active_clients = get_active_accounts(&payments);
        assert_eq!(
            active_clients,
            vec![(
                0,
                Account {
                    total: dec!(10.0),
                    held: dec!(0),
                    locked: false,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_withdraw_dispute() {
        let mut payments = Payments::default();
        let transactions = vec![
            Transaction {
                cid: 0,
                tid: 0,
                kind: TransactionKind::Deposit { amount: dec!(10.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Withdrawal { amount: dec!(5.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Dispute,
            },
        ];

        for transaction in transactions {
            payments.process_transaction(transaction);
        }

        let active_clients = get_active_accounts(&payments);
        assert_eq!(
            active_clients,
            vec![(
                0,
                Account {
                    total: dec!(10.0),
                    held: dec!(5.0),
                    locked: false,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_dispute_idempotency() {
        let mut payments = Payments::default();
        let transactions = vec![
            Transaction {
                cid: 0,
                tid: 0,
                kind: TransactionKind::Deposit { amount: dec!(10.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Withdrawal { amount: dec!(5.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Dispute,
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Dispute,
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Dispute,
            },
        ];

        for transaction in transactions {
            payments.process_transaction(transaction);
        }

        let active_clients = get_active_accounts(&payments);
        assert_eq!(
            active_clients,
            vec![(
                0,
                Account {
                    total: dec!(10.0),
                    held: dec!(5.0),
                    locked: false,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_withdraw_dispute_resolve() {
        let mut payments = Payments::default();
        let transactions = vec![
            Transaction {
                cid: 0,
                tid: 0,
                kind: TransactionKind::Deposit { amount: dec!(10.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Withdrawal { amount: dec!(5.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Dispute,
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Resolve,
            },
        ];

        for transaction in transactions {
            payments.process_transaction(transaction);
        }

        let active_clients = get_active_accounts(&payments);
        assert_eq!(
            active_clients,
            vec![(
                0,
                Account {
                    total: dec!(10.0),
                    held: dec!(0.0),
                    locked: false,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_withdraw_dispute_chargeback() {
        let mut payments = Payments::default();
        let transactions = vec![
            Transaction {
                cid: 0,
                tid: 0,
                kind: TransactionKind::Deposit { amount: dec!(10.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Withdrawal { amount: dec!(5.0) },
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Dispute,
            },
            Transaction {
                cid: 0,
                tid: 1,
                kind: TransactionKind::Chargeback,
            },
        ];

        for transaction in transactions {
            payments.process_transaction(transaction);
        }

        let active_clients = get_active_accounts(&payments);
        assert_eq!(
            active_clients,
            vec![(
                0,
                Account {
                    total: dec!(5.0),
                    held: dec!(0.0),
                    locked: true,
                    has_activity: true
                }
            )]
        );
    }
}
