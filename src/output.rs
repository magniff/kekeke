use rust_decimal::Decimal;
use serde::{Serialize, Serializer};

// Though it's not strickly required, lets keep our output nice and tidy
fn serialize_decimal_4dp<S>(value: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{:.4}", value.round_dp(4)))
}

#[derive(Debug, Serialize)]
pub struct OutputRow {
    pub client: u16,

    #[serde(serialize_with = "serialize_decimal_4dp")]
    pub available: Decimal,

    #[serde(serialize_with = "serialize_decimal_4dp")]
    pub held: Decimal,

    #[serde(serialize_with = "serialize_decimal_4dp")]
    pub total: Decimal,

    pub locked: bool,
}
