# Kekeke

This repository contains a toy transaction-processing engine for a simplified financial system.
It takes a CSV file of transactions as input and produces the resulting state of each account.

Example usage:

```bash
$ cargo r -r -- sample.csv
   Compiling kekeke v0.1.0 (/home/magniff/workspace/kekeke)
    Finished `release` profile [optimized] target(s) in 0.28s
     Running `target/release/kekeke sample.csv`
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

## Transaction types

The engine supports the following transaction types:

- deposit - adds funds to a client’s account
- withdrawal - removes funds from a client’s available balance
- dispute - marks a transaction as disputed, temporarily holding the associated funds
- resolve - reverts the disputed transaction
- chargeback - commits the disputed transaction and locks up the account
