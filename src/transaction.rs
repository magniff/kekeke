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

#[derive(Debug, Clone)]
pub struct Action {
    pub cid: u16,
    pub kind: ActionKind,
    pub disputed: bool,
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
