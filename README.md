# Kekeke
This repository contains a toy transaction-processing engine for a simplified financial system.
It consumes a CSV file of transactions and outputs the final state of all client accounts.

Example
```
$ cargo r -r -- sample.csv
   Compiling kekeke v0.1.0 (/home/magniff/workspace/kekeke)
    Finished `release` profile [optimized] target(s) in 0.28s
     Running `target/release/kekeke sample.csv`
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

## Overview

> Disclaimer: The original specification is somewhat vague. What follows is my interpretation of the rules and the behavior I implemented based on that understanding.

The system models a pool of clients in a simplified banking or exchange-like environment. Conceptually, it consists of two main components:
- Accounts (clients)
- Transactions

There is no direct transfer of funds between clients (as required by the spec). Instead, all activity happens through individual deposits and withdrawals. To make things more interesting, the system also supports transaction disputes, allowing clients to challenge previous transactions and potentially reverse or finalize them.

A disputed transaction can later be:
- Resolved (reverted in the client’s favor), or
- Charged back (forced to commit permanently), which also locks the client’s account.

## Account Model

Each account tracks two balances:
- `funds_total` — the total balance
- `funds_held` — funds currently locked due to disputes

The available balance is always:
```
funds_available = funds_total - funds_held
```

## Transaction Types and Effects
### Deposit
- `funds_total += amount`
- `funds_held` unchanged

### Withdrawal
- If `funds_available >= amount: funds_total -= amount`
- `funds_held` unchanged

## Dispute
Marks an existing transaction as disputed.

### Disputing a deposit
- `funds_total` remains unchanged
- `funds_held += transaction.amount`

This can result in `funds_held > funds_total`.
For example:
```
deposit 100
withdraw 50
withdraw 50
dispute deposit
resolve dispute
```

Final state:
```
funds_total = -100
funds_held = 0
```

In this case, the client ends up owing money to the system. This behavior is intentional and considered valid.

### Disputing a withdrawal
- `funds_total += transaction.amount`
- `funds_held += transaction.amount`

The withdrawn funds are temporarily returned but locked until the dispute is resolved.

## Resolve
Resolves a dispute in the client’s favor and reverts the disputed transaction.

### Resolving a disputed deposit
- `funds_total -= transaction.amount`
- `funds_held -= transaction.amount`

The deposit is effectively undone. If intervening withdrawals occurred, this may lead to a negative balance.

### Resolving a disputed withdrawal
- `funds_held -= transaction.amount`
- `funds_total` unchanged

The previously locked funds are simply released.

## Chargeback
Forces the disputed transaction to remain permanent and locks the client’s account for investigation.

### Chargeback on a deposit
- `funds_held -= transaction.amount`
- `funds_total` unchanged

The deposit is finalized, and the account is frozen.

### Chargeback on a withdrawal
- `funds_held -= transaction.amount`
- `funds_total -= transaction.amount`

The system confirms the withdrawal, removes the held funds, and locks the account.
Account is locked

## Behavioral scenarios

The following scenarios illustrate how the system behaves under different sequences of transactions. Each scenario corresponds directly to a test case in the codebase.

---

### Scenario: Multiple Deposits

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` deposits `20.0`

**Result**
- Total funds: `30.0`
- Held funds: `0.0`
- Account is not locked

---

### Scenario: Deposit Dispute

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` disputes the deposit

**Result**
- Total funds: `10.0`
- Held funds: `10.0`
- Funds are frozen but the account remains active

---

### Scenario: Deposit Dispute Resolved

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` disputes the deposit
3. Client `0` resolves the dispute

**Result**
- Total funds: `0.0`
- Held funds: `0.0`
- The deposit is fully reverted

---

### Scenario: Deposit Dispute with Chargeback

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` disputes the deposit
3. Client `0` issues a chargeback

**Result**
- Total funds: `10.0`
- Held funds: `0.0`
- Account is locked
- The deposit is finalized and can no longer be disputed

---

### Scenario: Successful Withdrawal

**Steps**
1. Client `0` deposits `20.0`
2. Client `0` withdraws `15.0`

**Result**
- Total funds: `5.0`
- Held funds: `0.0`
- Account remains active

---

### Scenario: Withdrawal Overdraft Is Ignored

**Steps**
1. Client `0` deposits `20.0`
2. Client `0` attempts to withdraw `25.0`

**Result**
- Withdrawal is silently ignored
- Total funds: `20.0`
- Held funds: `0.0`

---

### Scenario: Withdrawal Dispute

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` withdraws `5.0`
3. Client `0` disputes the withdrawal

**Result**
- Total funds: `10.0`
- Held funds: `5.0`
- Withdrawn funds are temporarily returned but locked

---

### Scenario: Withdrawal Dispute Resolved

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` withdraws `5.0`
3. Client `0` disputes the withdrawal
4. Client `0` resolves the dispute

**Result**
- Total funds: `10.0`
- Held funds: `0.0`
- Funds are fully restored and unfrozen

---

### Scenario: Withdrawal Dispute with Chargeback

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` withdraws `5.0`
3. Client `0` disputes the withdrawal
4. Client `0` issues a chargeback

**Result**
- Total funds: `5.0`
- Held funds: `0.0`
- Account is locked
- Withdrawal is permanently enforced

---

### Scenario: Failed Transactions Cannot Be Disputed

**Steps**
1. Client `0` deposits `10.0`
2. Client `0` attempts to withdraw `20.0` (fails)
3. Client `0` disputes the failed withdrawal
4. Client `0` resolves the dispute

**Result**
- Dispute and resolve have no effect
- Total funds: `10.0`
- Held funds: `0.0`
- Only successfully applied transactions are disputable

