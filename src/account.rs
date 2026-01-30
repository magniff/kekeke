use rust_decimal::Decimal;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Account {
    pub total: Decimal,
    pub held: Decimal,
    pub is_locked: bool,
    // to filter out relevant acc for output
    pub has_activity: bool,
}

impl Account {
    pub fn get_available(&self) -> Decimal {
        self.total - self.held
    }
}
