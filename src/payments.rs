use std::collections::HashMap;

use crate::{Account, Action, ActionKind, Transaction, TransactionKind};

pub struct Payments {
    pub accounts: Vec<Account>,
    pub actions: HashMap<u32, Action>,
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
        // NOTE: we are about to store the transaction for later, and as a storage key
        // we are using the tid - transaction id.
        // We are not gonna sanitize it in any way here, according to the spec they
        // suppose to be unique numbers
        match transaction.kind {
            // Processing deposits
            TransactionKind::Deposit { amount } => {
                account.total += amount;
                self.actions.insert(
                    transaction.tid,
                    Action {
                        cid: transaction.cid,
                        kind: ActionKind::Deposit { amount },
                        disputed: false,
                    },
                );
            }

            // Processing withdrawals
            TransactionKind::Withdrawal { amount } => {
                if account.get_available() >= amount {
                    account.total -= amount;
                    self.actions.insert(
                        transaction.tid,
                        Action {
                            cid: transaction.cid,
                            kind: ActionKind::Withdrawal { amount },
                            disputed: false,
                        },
                    );
                }
            }
            // Processing dispute situations
            TransactionKind::Dispute | TransactionKind::Resolve | TransactionKind::Chargeback => {
                // Check if we've seen that transaction before
                let Some(action) = self.actions.get_mut(&transaction.tid) else {
                    return;
                };
                // Checking if that transaction belonged to the client
                if action.cid != transaction.cid {
                    return;
                }

                match transaction.kind {
                    TransactionKind::Dispute => {
                        // Skipping if already disputed
                        if action.disputed {
                            return;
                        }
                        // This transaction is sus now, watch out
                        action.disputed = true;
                        match action {
                            // Disputing a withdrawal transaction
                            // What it means:
                            // - the total amount should become += transaction.amount
                            // - held amount should also go += transaction.amount
                            // - available funds are still the same
                            // meaning: the client might have not withdrew,
                            // but we'll keep those funds frozen for now
                            Action {
                                kind: ActionKind::Withdrawal { amount },
                                ..
                            } => {
                                let amount = *amount;
                                let account = self.get_account_mut(transaction.cid);
                                account.total += amount;
                                account.held += amount;
                            }
                            // Disputing a deposit transaction
                            // What it means:
                            // - the total amount should stay the same
                            // - held amount should go += transaction.amount
                            // - available amount should go -= transaction.amount
                            // meaning: the client might have not deposited, so lets lock those funds for now
                            // but we'll keep the total amount the same
                            // making their available pool lower
                            Action {
                                kind: ActionKind::Deposit { amount },
                                ..
                            } => {
                                let amount = *amount;
                                let account = self.get_account_mut(transaction.cid);
                                account.held += amount;
                            }
                        }
                    }
                    TransactionKind::Resolve => {
                        // Cant resolve what's not disputed, right?
                        if !action.disputed {
                            return;
                        }
                        action.disputed = false;
                        match action {
                            // Resolving a withdrawal transaction, reverting the transaction
                            // What it means:
                            // - the total amount should still be the same
                            // - held amount should also go -= transaction.amount, as those funds are not longer held
                            // - available amount should go += transaction.amount, as now those funds are no longer locked
                            // meaning: reverting the transaction,
                            // unfreezing the held funds and keeping total the same
                            Action {
                                kind: ActionKind::Withdrawal { amount },
                                ..
                            } => {
                                let amount = *amount;
                                let account = self.get_account_mut(transaction.cid);
                                account.held -= amount;
                            }
                            // Resolving a deposit transaction, reverting the transaction
                            // What it means:
                            // - the total amount should just go -= transaction.amount, pretending that
                            // the client never deposited
                            // - held amount should also go -= transaction.amount, as those funds are not longer held
                            // meaning: reverting the transaction,
                            Action {
                                kind: ActionKind::Deposit { amount },
                                ..
                            } => {
                                let amount = *amount;
                                let account = self.get_account_mut(transaction.cid);
                                account.total -= amount;
                                account.held -= amount;
                            }
                        }
                    }
                    TransactionKind::Chargeback => {
                        // Cant resolve what's not disputed, right?
                        if !action.disputed {
                            return;
                        }
                        action.disputed = false;
                        match action {
                            // Charging back a withdrawal transaction: forcing the transaction
                            // What it means:
                            // - the total amount should go -= transaction.amount, as the client is forced to pay
                            // - held amount should also go -= transaction.amount, as those funds are not longer held
                            // - available amount should thus be the same, as the client have already payed
                            Action {
                                kind: ActionKind::Withdrawal { amount },
                                ..
                            } => {
                                let amount = *amount;
                                let account = self.get_account_mut(transaction.cid);
                                account.held -= amount;
                                account.total -= amount;
                                account.locked = true;
                            }
                            // Charging back a deposit transaction: forcing the transaction
                            // What it means:
                            // - the total amount should stay the same
                            // - held amount should also go -= transaction.amount, as those funds are not longer held
                            // - available amount should thus go += transaction.amount, as now the client has more funds
                            Action {
                                kind: ActionKind::Deposit { amount },
                                ..
                            } => {
                                let amount = *amount;
                                let account = self.get_account_mut(transaction.cid);
                                account.held -= amount;
                                account.locked = true;
                            }
                        }
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
    fn test_deposit_dispute() {
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
                    total: dec!(30),
                    held: dec!(20),
                    locked: false,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_deposit_dispute_rollback() {
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
                    total: dec!(30),
                    held: dec!(0),
                    locked: true,
                    has_activity: true
                }
            )]
        );
    }

    #[test]
    fn test_deposit_dispute_resolve() {
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
                    total: dec!(10),
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
