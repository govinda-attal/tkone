# Contract engine — The core domain

Status: **architecture only**. Every model the three examples use, declared
in one place: the [fixed-term deposit](example-fixed-deposit.md), the
[personal loan](example-personal-loan.md) and the
[offset mortgage](example-advanced-offset-mortgage.md). The rules these
declarations follow are in [models](contract-models.md); the actions and the
ledger extension in [hooks](contract-hooks.md). Where a sketch elsewhere
differs, this document is the reference. The sketches are illustrative, not
APIs.

## The shape

```text
                 ┌──────────────────────────── engine ────────────────────────────┐
                 │ kernel primitives: Append, Set, Create/End, Transition,         │
                 │                    Deliver, ScheduleChange                      │
                 │ extension: ledger@1   (ledger.book, ledger.reverse)             │
                 └─────────────────────────────────▲──────────────────────────────┘
                                                   │ requires ledger@^1
                 ┌─────────────────────────── core 1.5 ───────────────────────────┐
                 │ parties · accounts · roles · ledger · holds · reference data    │
                 └───────▲───────────────────────▲─────────────────────▲──────────┘
                         │ extends Account       │ extends Account     │ extends Account
          ┌──────────────┴───┐        ┌──────────┴─────────┐   ┌───────┴──────────┐
          │ deposits 3.2     │        │ term-deposits 1.0  │   │ lending 1.1      │
          │ DepositTerms     │        │ DepositTerms       │   │ LoanTerms,       │
          │ Accrued          │        │ Principal, Accrued │   │ Security, offsets│
          └──────▲───────────┘        └─────────▲──────────┘   │ holidays,        │
                 │                              │              │ collection       │
     easy-saver, offset-saver              fixed-12m           └──▲──────────▲────┘
                                                                  │          │
                                                       personal-loan   offset-tracker
```

| Model | Version | Requires | Used by | Owned by |
|---|---|---|---|---|
| `core` | 1.5.0 | extension `ledger@^1` | every product | the platform's modelling team |
| `deposits` | 3.2.0 | `core@^1.5` | `easy-saver`, `offset-saver` | deposits |
| `term-deposits` | 1.0.0 | `core@^1.5` | `fixed-12m` | deposits |
| `lending` | 1.1.0 | `core@^1.5` | `personal-loan`, `offset-tracker` | lending |

The core is small on purpose (C15): what every product means the same way.
Each domain adds facets to `core::Account`, its own balance types, its own
actions and views, and never changes what a core type means.

---

## `core`

```rust
#[model(name = "core", version = "1.5.0", extensions = ["ledger@^1"])]
mod core {
    // ───────────────────────────── Parties ─────────────────────────────

    /// A person or an organisation. A customer, a guarantor, an attorney are
    /// roles a party plays, not kinds of party.
    #[entity(key = party_id, subject = party_id, retain = "after(closed) + 7y")]
    #[identifiers]                                   // passport, national id, LEI, CRM id …
    struct Party {
        party_id: PartyId,
        kind: PartyKind,                             // Person | Organisation
        #[personal] #[effective] legal_name: Name,
        #[personal(special)] date_of_birth: Option<NaiveDate>,
        #[personal] #[effective] address: Option<PostalAddress>,
        #[personal(search = "exact")] #[effective] email: Option<Email>,
        #[effective] merged_into: Option<PartyId>,   // a merge is a redirect
    }

    // ───────────────────────────── Accounts ────────────────────────────

    /// One account, whatever product it is: a deposit, a loan, an internal
    /// account. Products add facets; they do not redefine it.
    #[entity(key = account_id)]
    #[identifiers]                                   // sort code and number, IBAN, loan number …
    struct Account {
        account_id: AccountId,
        product_line: ProductLine,                   // open code set: reference data
        legal_entity: LegalEntity,
        currency: Currency,
        opened_on: NaiveDate,
        #[effective] status: AccountStatus,
    }

    #[state_machine(Account.status, initial = "Pending")]
    transitions! {
        Pending -> Active | Closed,                  // closed unfunded
        Active  -> Dormant | Closed,
        Dormant -> Active | Closed,
    }

    /// Parties and accounts, many to many, each in a role.
    #[relation(Account <-> Party, max_per_party = 50, max_per_account = 20)]
    struct AccountPartyRole { #[effective] role: Role }   // Holder | JointHolder | Borrower
                                                          // | Guarantor | Attorney

    /// An agreed overdraft or other limit on what may be debited.
    #[effective(key = account_id)]
    #[action(set = "SetLimit")]
    #[invariant("overdraft.currency == account.currency")]
    struct Limit { account_id: AccountId, overdraft: Money }

    // ────────────────────────────── Ledger ─────────────────────────────

    /// The balance types every account has. Domains add their own; a balance
    /// type is named with its model (`lending::Principal`).
    #[balances]
    enum CoreBalance { Available }

    /// Entries that net to zero, booked together. Its meaning needs code:
    /// the ledger extension's actions.
    #[fact(key = booking_id)]
    #[action(extension = "ledger.book", reverse = "ledger.reverse")]
    struct Booking {
        booking_id: BookingId,
        code: BankTransactionCode,                   // ISO 20022 domain, family, sub-family
        value_date: NaiveDate,                       // the booking date is the commit's
        reverses: Option<BookingId>,
        #[bounded(max = 1000)] entries: Vec<Entry>,
        #[personal(subject = "counterparty")] narrative: Option<Text>,
    }

    /// One leg: an amount, debit or credit, to one balance of one account.
    #[fact(key = account_id)]
    struct Entry {
        account_id: AccountId,
        booking_id: BookingId,
        address: BalanceAddress,                     // account, balance type, currency
        amount: Money,                               // or a balance type's declared scale
        side: Side,                                  // Debit | Credit
        value_date: NaiveDate,
    }

    /// Authorised, not booked: reduces the available balance.
    #[effective(key = account_id)]
    #[action(create = "Hold", end = "Release")]
    #[invariant("amount.currency == account.currency")]
    #[invariant("amount <= state.available", reject = "hold-exceeds-available")]
    struct Hold {
        hold_id: HoldId,
        account_id: AccountId,
        address: BalanceAddress,
        amount: Money,
        until: Option<NaiveDate>,
        reason: HoldReason,
    }

    /// An instance's override of a parameter, effective from a date (C8).
    #[effective(key = (account_id, name))]
    #[action(set = "SetParameter")]
    #[invariant("value >= parameter.min && value <= parameter.max", reject = "out-of-bounds")]
    struct ParameterOverride { account_id: AccountId, name: ParameterName, value: ParameterValue }

    // ────────────────────────── Reference data ─────────────────────────

    #[effective(key = code)]
    struct CurrencyInfo { code: Currency, minor_units: u8 }          // GBP 2, JPY 0, BHD 3

    #[effective(key = code)]
    struct ProductLineInfo { code: ProductLine, name: Text }         // savings, term-deposits,
                                                                     // personal-loans, mortgages
    #[series(key = code)]
    struct RateSeries { code: SeriesCode, rate: Rate }               // boe-base, SONIA

    #[effective(key = code)]
    struct RateCard { code: RateCardCode, #[bounded(max = 50)] tiers: Vec<RateTier> }

    #[effective(key = code)]
    struct LedgerAccount { code: LedgerAccountCode, name: Text, kind: LedgerKind }
                                                                     // Asset | Liability | Income
                                                                     // | Expense | Equity
    /// Contracts book to logical names; finance maps them, per legal entity.
    #[effective(key = (mapping, logical))]
    struct AccountingMapping {
        mapping: MappingCode,
        legal_entity: LegalEntity,
        logical: LogicalAccount,
        ledger_account: LedgerAccountCode,
    }

    #[effective(key = scheme)]
    struct IdentifierScheme {
        scheme: SchemeCode,
        applies_to: EntityName,
        when: Option<Condition>,
        format: Option<FormatCode>,
        unique: UniqueScope,                         // global | per-issuer | per-system | per-legal-entity
        per_object: Cardinality,                     // one | one-active | many
        issued_by: Issuer,                           // bank | external
        class: Classification,
    }

    // Calendars are reference data too, loaded through tkone-calendar.

    // ────────────────────────────── Views ──────────────────────────────

    #[view(key = account_id, consistency = "with-instance",
           latest_of = [Account.status, Limit.overdraft])]
    struct AccountStatus { account_id: AccountId, status: AccountStatus, overdraft: Money }

    #[view(key = account_id, consistency = "with-instance", from = Entry,
           group_by = address, fold = sum(signed(amount)))]
    struct Balances { account_id: AccountId, address: BalanceAddress, amount: Money }

    #[view(key = account_id, consistency = "with-instance", from = Hold,
           filter = "ended is none", fold = sum(amount))]
    struct HoldsTotal { account_id: AccountId, amount: Money }

    /// What a hook reads about its own account, before a domain adds to it.
    #[view(key = account_id, consistency = "with-instance",
           compose = [AccountStatus, Balances, HoldsTotal],
           derive = { available: "Balances[Available] - HoldsTotal + AccountStatus.overdraft" })]
    struct AccountState;

    #[view(key = party_id, cluster = account_id, consistency = "eventual",
           from = AccountPartyRole)]
    struct AccountsByParty { party_id: PartyId, account_id: AccountId, role: Role }
}
```

### The ledger's vocabulary

| Term | Means |
|---|---|
| **Booking** | entries that net to zero, booked together; what a hook returns as `Book` |
| **Entry** | one leg, to one balance address, with a value date and a booking date |
| **Reversal** | a booking that undoes another and names it |
| **Hold** | authorised, not booked; reduces the available balance |
| **Balance address** | account, balance type, currency |
| **Home** | the instance whose operation produced a booking, where guarded debits are checked |
| **Delivery** | another account's entries, inserted into its inbox ([hot accounts](hot-accounts.md)) |
| **Logical account** | a name a contract books to (`loan_interest_income`), mapped to a ledger account |

### Transaction codes the examples use

Closed code set, from ISO 20022's bank transaction codes:

| Code (in the SDK) | Used for | By |
|---|---|---|
| `DEPOSIT`, `CASH_WITHDRAWAL`, `TRANSFER` | money in and out of deposits | `easy-saver`, `offset-saver` |
| `INTEREST_ACCRUAL`, `MATURITY` | a deposit's interest and payout | `fixed-12m` |
| `LOAN_DRAWDOWN` | paying a loan out | `personal-loan`, `offset-tracker` |
| `LOAN_INTEREST_ACCRUAL`, `LOAN_INSTALMENT`, `LOAN_FEE` | a loan's interest, instalments, charges | `personal-loan`, `offset-tracker` |
| `LOAN_REPAYMENT`, `LOAN_OVERPAYMENT` | allocating what a loan receives | `personal-loan`, `offset-tracker` |

---

## `deposits`

Instant-access savings, and the offset saver.

```rust
#[model(name = "deposits", version = "3.2.0", requires = "core@^1.5")]
mod deposits {
    #[balances]
    enum DepositBalance { #[balance(scale = 12)] Accrued }   // interest owed, until capitalised

    #[extends(core::Account, when = "product_line = 'savings'")]
    struct DepositTerms {
        #[deprecated(since = "3.2.0", use = "notice")]
        #[effective] notice_days: u16,
        #[effective] notice: Option<NoticePeriod>,           // calendar or business days
    }

    #[view(key = account_id, consistency = "with-instance",
           compose = [core::AccountState],
           derive = { accrued: "core::Balances[Accrued]" })]
    struct SavingsState;
}
```

## `term-deposits`

```rust
#[model(name = "term-deposits", version = "1.0.0", requires = "core@^1.5")]
mod term_deposits {
    #[balances]
    enum TermDepositBalance { Principal, #[balance(scale = 12)] Accrued }

    #[extends(core::Account, when = "product_line = 'term-deposits'")]
    struct DepositTerms {
        matures_on: NaiveDate,
        #[effective] #[action(set = "SetPayTo")]
        pay_to: AccountId,                                   // where maturity pays out
    }

    #[view(key = account_id, consistency = "with-instance", from = core::Entry,
           group_by = address, fold = sum(signed(amount)))]
    struct DepositBalances { account_id: AccountId, address: BalanceAddress, amount: Money }
}
```

## `lending`

Every loan product: the personal loan uses its terms, balances and collection
status; the mortgage also its security, offsets and payment holidays.

```rust
#[model(name = "lending", version = "1.1.0", requires = "core@^1.5")]
mod lending {
    #[balances]
    enum LoanBalance {
        Principal,
        #[balance(scale = 12)] Accrued,              // interest, finer than pennies
        Due,                                         // instalments fallen due, unpaid
        Fees,
        Incoming,                                    // payments, until allocated
    }

    #[extends(core::Account, when = "product_line in ['mortgages', 'personal-loans']")]
    struct LoanTerms {
        #[effective] term_end: NaiveDate,
        #[effective] repayment_day: u8,
        #[effective] repayment: RepaymentType,       // Repayment | InterestOnly
        #[effective] collection: Option<CollectionStatus>,      // 1.1
    }

    #[state_machine(LoanTerms.collection, initial = "Current")]  // 1.1
    transitions! { Current -> Arrears, Arrears -> Current | Default }

    /// What secures a loan: mortgages only.
    #[extends(core::Account, when = "product_line = 'mortgages'")]
    struct Security { property: PropertyRef, #[effective] valuation: Money }

    /// Savings accounts whose balances offset a mortgage's interest.
    #[relation(Account as mortgage <-> Account as offset, max_per_mortgage = 5, unique = "offset")]
    #[action(create = "LinkOffset", end = "UnlinkOffset")]
    #[invariant("input.offset.status == 'Active'", reject = "offset-not-active")]
    #[invariant("input.offset.holders.all(h, h in state.borrowers)", reject = "not-the-borrowers")]
    struct OffsetLink { #[effective] since: NaiveDate }

    #[effective(key = account_id)]
    #[action(create = "StartPaymentHoliday", end = "EndPaymentHoliday")]
    #[invariant("months >= 1 && months <= 6")]
    #[invariant("state.holiday_months_taken + months <= 6", reject = "holiday-allowance-used")]
    struct PaymentHoliday { account_id: AccountId, from: NaiveDate, months: u8 }

    #[view(key = account_id, consistency = "with-instance",
           from = core::Entry, filter = "code == 'LOAN_OVERPAYMENT' && value_date >= state.year_start",
           fold = sum(amount))]
    struct OverpaidThisYear { account_id: AccountId, amount: Money }

    #[view(key = account_id, consistency = "with-instance",
           from = PaymentHoliday, fold = sum(months))]
    struct HolidayMonthsTaken { account_id: AccountId, months: u8 }

    #[view(key = account_id, consistency = "with-instance",
           compose = [core::AccountState, OverpaidThisYear, HolidayMonthsTaken],
           derive = {
               principal:   "core::Balances[Principal]",
               accrued:     "core::Balances[Accrued]",
               due:         "core::Balances[Due]",
               fees:        "core::Balances[Fees]",
               outstanding: "principal + due + fees",
           })]
    struct LoanState;

    #[view(key = offset_id, consistency = "eventual", from = OffsetLink)]
    struct OffsetOf { offset_id: AccountId, mortgage_id: AccountId }
}
```

---

## Reference data the examples load

| Kind | Code | Values | Used by |
|---|---|---|---|
| Currency | `GBP` | 2 minor units | all |
| Calendar | `GBLO` | London business days | every schedule |
| Rate series | `boe-base` | Bank Rate, by date | `offset-tracker` |
| Product lines | `savings`, `term-deposits`, `personal-loans`, `mortgages` | — | facets' `when` |
| Identifier schemes | `sort-code-account`, `iban`, `loan-number`, `mortgage-number`, `passport`, `lei` | per [models 9](contract-models.md#9-identities-are-typed-and-survive-merges) | opening |
| Accounting mappings | `deposits-uk`, `loans-uk`, `mortgages-uk` | logical names → ledger accounts | every booking |

| Logical account | Mapping | Booked by |
|---|---|---|
| `deposit_interest_expense` | `deposits-uk` | `fixed-12m`, `easy-saver` accruals |
| `customer_withdrawals` | `deposits-uk` | `easy-saver` cash withdrawals |
| `loan_interest_income`, `loan_fee_income` | `loans-uk` | `personal-loan` |
| `mortgage_interest_income`, `mortgage_fee_income`, `erc_income`, `mortgage_drawdown` | `mortgages-uk` | `offset-tracker` |

## Products on the domain

| Definition | Model | Interface | Guard, credits land in | Actions it permits |
|---|---|---|---|---|
| `easy-saver@2.1` | `deposits` | `deposit-account@1` | debits; `Available` | `Book`, `Reverse`, `Hold`, `Release`, `Transition`, `ScheduleChange` |
| `offset-saver@1` | `deposits` | `deposit-account@1` | debits; `Available` | `Book`, `Reverse`, `Hold`, `Release`, `Transition` |
| `fixed-12m@1` | `term-deposits` | — | debits and credits; `Principal` | `Book`, `Transition`, `SetPayTo`, `ScheduleChange` |
| `personal-loan@1` | `lending` | — | debits; `Incoming` | `Book`, `SetParameter`, `Transition` (status, collection), `ScheduleChange` |
| `offset-tracker@1` | `lending` | `loan-account@1` | debits; `Incoming` | `Book`, `Reverse`, `LinkOffset`, `UnlinkOffset`, `StartPaymentHoliday`, `EndPaymentHoliday`, `Transition`, `ScheduleChange` |

## What is deliberately not in the core

- **Product rules:** interest methods, instalment formulas, allowances,
  charges. They are hook code, per product.
- **Anything one domain means differently:** "principal" is a deposit's and a
  loan's, so each declares its own (`term_deposits::Principal`,
  `lending::Principal`).
- **Payments and cards:** schemes, mandates, card authorisations. They would
  be domain models of their own (`payments`, `cards`), extending
  `core::Booking` with facets for their details, when a product needs them.
