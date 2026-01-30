use rust_decimal::Decimal;
use serde::{Deserialize, de::Deserializer};

#[derive(Debug)]
pub enum TransactionKind {
    Deposit { amount: Decimal },
    Withdrawal { amount: Decimal },
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug)]
pub struct Transaction {
    pub tid: u32,
    pub cid: u16,
    pub kind: TransactionKind,
}

#[derive(Debug, Clone)]
pub enum ActionKind {
    Deposit { amount: Decimal },
    Withdrawal { amount: Decimal },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionStatus {
    // All actions are born with status == fresh
    Fresh,
    // If the client ever tries to dispute the transaction
    // it becomes status == disputed
    Disputed,
    // After being resolved or charged back
    // it becomes status == final
    Final,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub cid: u16,
    pub kind: ActionKind,
    pub status: ActionStatus,
}

impl<'de> Deserialize<'de> for Transaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TransactionCSVRow<'a> {
            #[serde(rename = "type")]
            pub kind: &'a str,

            #[serde(rename = "client")]
            pub cid: u16,

            #[serde(rename = "tx")]
            pub tid: u32,

            pub amount: Option<Decimal>,
        }

        let row = TransactionCSVRow::deserialize(deserializer)?;
        let kind = match row.kind {
            "deposit" => {
                let amount = row
                    .amount
                    .ok_or_else(|| serde::de::Error::custom("deposit requires amount"))?;
                if amount <= Decimal::ZERO {
                    return Err(serde::de::Error::custom("deposit amount must be positive"));
                }
                TransactionKind::Deposit { amount }
            }
            "withdrawal" => {
                let amount = row
                    .amount
                    .ok_or_else(|| serde::de::Error::custom("withdrawal requires amount"))?;
                if amount <= Decimal::ZERO {
                    return Err(serde::de::Error::custom(
                        "withdrawal amount must be positive",
                    ));
                }
                TransactionKind::Withdrawal { amount }
            }
            "dispute" => {
                if row.amount.is_some() {
                    return Err(serde::de::Error::custom("dispute must not have amount"));
                }
                TransactionKind::Dispute
            }
            "resolve" => {
                if row.amount.is_some() {
                    return Err(serde::de::Error::custom("resolve must not have amount"));
                }
                TransactionKind::Resolve
            }
            "chargeback" => {
                if row.amount.is_some() {
                    return Err(serde::de::Error::custom("chargeback must not have amount"));
                }
                TransactionKind::Chargeback
            }
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "unknown transaction type: {}",
                    row.kind
                )));
            }
        };

        Ok(Transaction {
            cid: row.cid,
            tid: row.tid,
            kind,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv::ReaderBuilder;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn parse_single(csv: &str) -> Result<Transaction, csv::Error> {
        let mut rdr = ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(csv.as_bytes());

        let mut iter = rdr.deserialize::<Transaction>();
        iter.next().unwrap()
    }

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    // -------------------------
    // Happy-path parsing
    // -------------------------

    #[test]
    fn parse_deposit() {
        let tx = parse_single(
            "type,client,tx,amount\n\
             deposit,1,100,1.2345",
        )
        .unwrap();

        assert_eq!(tx.cid, 1);
        assert_eq!(tx.tid, 100);

        match tx.kind {
            TransactionKind::Deposit { amount } => {
                assert_eq!(amount, dec("1.2345"));
            }
            _ => panic!("expected deposit"),
        }
    }

    #[test]
    fn parse_withdrawal() {
        let tx = parse_single(
            "type,client,tx,amount\n\
             withdrawal,2,200,10.00",
        )
        .unwrap();

        match tx.kind {
            TransactionKind::Withdrawal { amount } => {
                assert_eq!(amount, dec("10.00"));
            }
            _ => panic!("expected withdrawal"),
        }
    }

    #[test]
    fn parse_dispute() {
        let tx = parse_single(
            "type,client,tx\n\
             dispute,3,300",
        )
        .unwrap();

        matches!(tx.kind, TransactionKind::Dispute);
    }

    #[test]
    fn parse_resolve() {
        let tx = parse_single(
            "type,client,tx\n\
             resolve,4,400",
        )
        .unwrap();

        matches!(tx.kind, TransactionKind::Resolve);
    }

    #[test]
    fn parse_chargeback() {
        let tx = parse_single(
            "type,client,tx\n\
             chargeback,5,500",
        )
        .unwrap();

        matches!(tx.kind, TransactionKind::Chargeback);
    }

    // -------------------------
    // Validation errors
    // -------------------------

    #[test]
    fn deposit_requires_amount() {
        let err = parse_single(
            "type,client,tx\n\
             deposit,1,1",
        )
        .unwrap_err();

        assert!(err.to_string().contains("deposit requires amount"));
    }

    #[test]
    fn withdrawal_requires_amount() {
        let err = parse_single(
            "type,client,tx\n\
             withdrawal,1,1",
        )
        .unwrap_err();

        assert!(err.to_string().contains("withdrawal requires amount"));
    }

    #[test]
    fn deposit_amount_must_be_positive() {
        let err = parse_single(
            "type,client,tx,amount\n\
             deposit,1,1,0",
        )
        .unwrap_err();

        assert!(err.to_string().contains("deposit amount must be positive"));
    }

    #[test]
    fn withdrawal_amount_must_be_positive() {
        let err = parse_single(
            "type,client,tx,amount\n\
             withdrawal,1,1,-5",
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("withdrawal amount must be positive")
        );
    }

    #[test]
    fn dispute_must_not_have_amount() {
        let err = parse_single(
            "type,client,tx,amount\n\
             dispute,1,1,1.0",
        )
        .unwrap_err();

        assert!(err.to_string().contains("dispute must not have amount"));
    }

    #[test]
    fn resolve_must_not_have_amount() {
        let err = parse_single(
            "type,client,tx,amount\n\
             resolve,1,1,1.0",
        )
        .unwrap_err();

        assert!(err.to_string().contains("resolve must not have amount"));
    }

    #[test]
    fn chargeback_must_not_have_amount() {
        let err = parse_single(
            "type,client,tx,amount\n\
             chargeback,1,1,1.0",
        )
        .unwrap_err();

        assert!(err.to_string().contains("chargeback must not have amount"));
    }

    #[test]
    fn unknown_transaction_type() {
        let err = parse_single(
            "type,client,tx,amount\n\
             magic,1,1,10.0",
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown transaction type"));
    }

    // -------------------------
    // Precision preservation
    // -------------------------

    #[test]
    fn decimal_precision_is_preserved() {
        let tx = parse_single(
            "type,client,tx,amount\n\
             deposit,1,1,1.0000000001",
        )
        .unwrap();

        match tx.kind {
            TransactionKind::Deposit { amount } => {
                assert_eq!(amount.to_string(), "1.0000000001");
            }
            _ => panic!("expected deposit"),
        }
    }
}
