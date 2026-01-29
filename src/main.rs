use clap::Parser;
use csv::{ReaderBuilder, Writer};
use std::io;

mod transaction;
use transaction::{Action, ActionKind, Transaction, TransactionKind};

mod payments;
use payments::Payments;

mod account;
use account::Account;

mod output;
use output::OutputRow;

#[derive(Parser)]
struct Options {
    path: std::path::PathBuf,
}

fn process_csv(payments: &mut Payments, input_path: &str) -> anyhow::Result<()> {
    for result in ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(input_path)?
        .deserialize::<Transaction>()
    {
        match result {
            Ok(transaction) => payments.process_transaction(transaction),
            // According to the spec we are not suppose to fatal the process should we encounter a
            // faulty transaction, so, we'll just complain and proceed
            Err(deserialization_error) => {
                eprintln!("Warning: Failed to parse transaction: {deserialization_error}")
            }
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let options = Options::parse();
    let mut payments = Payments::default();

    // Processing all the transactions from the input file,
    // mutating the state of the payments instance
    process_csv(
        &mut payments,
        options
            .path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("The path path to the input is invalid"))?,
    )?;

    // Filtering out only the accounts that actually participated
    // and building the output stream from them
    let output_stream = payments
        .accounts
        .iter()
        .enumerate()
        .filter(|(_, account)| account.has_activity)
        .map(|(client_id, account)| OutputRow {
            client: client_id as u16,
            available: account.total - account.held,
            held: account.held,
            total: account.total,
            locked: account.locked,
        });

    // Actually writing the output to stdout
    let mut writer = Writer::from_writer(io::stdout());
    for account in output_stream {
        writer.serialize(account)?;
    }
    writer.flush()?;

    Ok(())
}
