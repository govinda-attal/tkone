# Contract engine — Design

Status: **architecture only**. Nothing here is built. It describes the crates
and apps that would run a core banking product (a savings account, a loan) as
versioned contracts over versioned models, with [tempo](schedule-document.md)
running their schedules. The code sketches are illustrative, not APIs.

Companion documents: [models](contract-models.md) (the rules every model
follows, with what an author writes for each, and the forms of views),
[hooks, outcomes and events](contract-hooks.md) (what a hook receives and
returns, every action's shape, events, results and queries), [hot
accounts](hot-accounts.md) (how the ledger avoids serialising busy accounts)
and [extending the base](extending-the-base.md) (how a bank adds fields,
operations, events and behaviour to the models and products the platform
ships, without forking them).

**Start with an example,** each one product from its model to its customers,
in the order the work is done: a [fixed-term deposit](example-fixed-deposit.md)
first, then a [personal loan](example-personal-loan.md), then an [offset
mortgage](example-advanced-offset-mortgage.md), which reads other accounts and
runs for twenty-five years. The [core domain](core-domain.md) declares every
model the three use, in one place. This document and its companions explain
each piece they use.

## Motivation

A banking product is mostly rules about an account's life: what may be
deposited, how interest accrues and when it is capitalised, what a repayment
holiday does, how the account closes. Those rules change by product, by
version, and sometimes by account. They should be:

- **written by people who know the product,** against a model of the domain,
  without writing to the database themselves;
- **fast,** because the data they need is computed before they run;
- **versioned and auditable:** which code, with which values, decided this
  entry, and the same answer on replay;
- **scheduled** by tempo, with the account's own dates.

The paradigm is a *smart contract* without a blockchain. A contract is a pure
function: given an operation, the account's prepared data and its parameters,
it returns the **actions** it wants. The engine checks them, and writes them
to the entities, the ledger and every view that depends on them.

The paradigm is proven: products as versioned contracts, accounts bound to a
version, hooks that receive the data they declared and return what they want
done instead of writing. Its usual weak points, the upgrade of live accounts
and the limits on what a hook can see, are decided here on purpose (C9, C5).

---

## Layers

| Layer | Owns | Lives in | Made available by |
|---|---|---|---|
| **Model** | Entities, relations, views (read shapes), invariants, with a version | Rust crates | `tkone model apply`: DDL, and a row in the model registry |
| **Definition** | A product's interface: operations, hooks, permitted actions, parameters and defaults, schedules, the model version it needs | Rust crates (and a WIT package) | registration: its defaults, schedules and version persisted |
| **Implementation** | Code for one definition version: the hooks, their data requirements, its own parameter values and schedules | a WASM component in an OCI registry, signed | registration by digest, then promotion (C7) |
| **Instance** | One account bound to an implementation: its overrides, its state, its schedule instances | the database | the engine, when the account is opened |
| **Engine** | Running hooks: resolve, fetch, invoke, validate, commit | `tkone-engine`, an app | — |

Models and definitions have no execution of their own: they are crates
because code is where types and checks belong, and they are persisted because
the engine and the implementations need their versions and defaults at run
time. Implementations execute, so they are artifacts, pinned by digest.

```text
 crates                     database (registries)               OCI registry
┌───────────────┐  apply   ┌────────────────────────────────┐
│ model vN      │────────► │ model versions, applied DDL     │
├───────────────┤ register │ definitions: parameters,        │
│ definition vN │────────► │   defaults, schedules (tempo)   │
└───────────────┘          │ implementations: digest, state, │ ◄─── wasm component,
                           │   values, manifest              │      signed, by digest
                           │ instances: binding, overrides   │
                           └───────────────┬────────────────┘
      operations, bulk ───────►      ENGINE      ◄─────── tempo: due runs, with sources
                         resolve → fetch → invoke → validate → commit
```

---

## Models

A model crate declares entities, their relations, and **views**: read shapes
keyed for the way hooks and APIs read them, as a ScyllaDB table is designed
per query.

### A core, and domains that extend it

Some types belong to no single product: a party, an account, the ledger.
They live in one **core** model, small and stable. Domain models (deposits,
lending, cards) depend on a range of its versions and **extend** its
entities; they never redefine them (C15).

```text
core
├── party       Party (person or organisation), PartyRole, contact points
├── account     Account, AccountPartyRole (holder, joint holder, guarantor),
│               status, product binding
├── ledger      LedgerAccount (chart of accounts), Booking, Entry, Hold,
│               balance addresses
└── reference   currencies, calendars, rate indices, rate cards, FX
deposits  ── extends core::Account ──►  DepositTerms (notice period, access)
lending   ── extends core::Account ──►  LoanTerms (principal, term)
```

```rust
#[model(name = "core", version = "1.4.0", extensions = ["ledger@^1"])]
mod core {
    #[entity(key = party_id)]
    struct Party { party_id: PartyId, kind: PartyKind, #[effective] name: Name }

    #[entity(key = account_id)]
    struct Account { account_id: AccountId, opened_on: NaiveDate, #[effective] status: Status }

    /// The statuses an account moves through; `Transition` follows it.
    #[state_machine(Account.status)]
    transitions! { Pending -> Active, Active -> Dormant | Closed, Dormant -> Active | Closed }

    /// Parties and accounts are many to many, each in a role.
    #[relation(Account <-> Party)]
    struct AccountPartyRole { #[effective] role: Role }

    /// Entries that net to zero, booked together. Booking is the ledger
    /// extension's action: its meaning needs code.
    #[fact(key = booking_id)]
    #[action(extension = "ledger.book", reverse = "ledger.reverse")]
    struct Booking { booking_id: BookingId, reverses: Option<BookingId>, entries: Vec<Entry> }

    /// One leg: an amount to one balance of one account.
    #[fact(key = account_id)]
    struct Entry { account_id: AccountId, address: BalanceAddress, amount: Money, side: Side }

    /// Authorised, not booked: reduces the available balance; not an entry.
    /// A declared action: a kernel primitive and its invariants.
    #[effective(key = account_id)]
    #[action(create = "Hold", end = "Release")]
    #[invariant("amount <= state.available", reject = "hold-exceeds-available")]
    struct Hold { hold_id: HoldId, address: BalanceAddress, amount: Money }
}

#[model(name = "deposits", version = "3.1.0", requires = "core@^1.4")]
mod deposits {
    /// A facet of a core account: one to one, keyed by its id.
    #[extends(core::Account, when = "product_line = 'deposits'")]
    struct DepositTerms { #[effective] notice_days: u16 }

    /// What a hook reads about its own account, kept with every commit.
    #[view(key = account_id, consistency = "with-instance")]
    struct AccountState { account_id: AccountId, status: Status, balances: Balances, accrued: Money }

    /// Read across accounts; may lag.
    #[view(key = party_id, cluster = account_id, consistency = "eventual")]
    struct AccountsByParty { party_id: PartyId, account_id: AccountId, available: Money }
}
```

- **Party, not customer.** A customer is a role a party plays; a guarantor, a
  joint holder or an attorney is the same party in another. Accounts and
  parties are many to many, with roles.
- **One account, whatever it carries.** A deposit's terms are a facet of the
  core account, so the account keeps one identity, one status history and
  one list of parties.
- **A model declares its actions,** not only its data: which kernel
  primitive each is (append a fact, set an attribute, create or end an
  entity, transition a state), with invariants the engine checks at commit.
  Meanings that need code (the ledger) come from an extension the model
  requires. The engine knows no domain; banking is the core model and the
  ledger extension ([actions](contract-hooks.md#actions), C28).
- **One source, two outputs.** A model crate generates the Rust types the
  engine uses and the WIT types implementations see, so the two cannot
  drift.
- **Apply.** `tkone model apply deposits@3.1.0` checks that the core range it
  requires is applied, diffs against the applied version, emits and runs DDL,
  and records the version. Models grow rather than break: fields are added
  and deprecated, never removed or retyped in place, and a breaking change is
  an approved exception ([models](contract-models.md#2-models-grow-they-do-not-break)).
  A major core version moves every domain, which is why the core stays small.
- **Views are derived, never written by contracts** (C4). Each is declared in
  one of four forms (re-key, latest, fold, compose), and the engine maintains
  it from the actions that change its sources
  ([views](contract-models.md#views-four-forms-no-code), C26).
- **Rules every model follows,** from its first version, because they cannot
  be added once data exists: stable field ids, growth instead of breaking,
  compatibility per kind, personal data classified and erasable, exact
  numbers, bounded collections, unserialised hot accounts, a schema named on
  every record, typed identities that survive merges. Each, with what its
  author writes: [models](contract-models.md).

### Names

Types are named in full English words (`Party`, `Account`,
`BalanceType::Available`), with the vocabulary taken from the industry's
interchange standards: IFX for parties, accounts and their relationships,
balance and account types; ISO 20022 for payments and its code sets. A
vocabulary page maps each core term and code to its equivalents, and where
messages are exchanged in those formats, serialisation renames to them. The
standards are sources of terms and codes, not schemas: their abbreviated
message names (`Acct`, `Amt`, `Dt`) and request and response structures are
not a storage model.

The ledger's words, chosen so each means one thing:

| Term | Means | IFX |
|---|---|---|
| **Entry** | one leg: an amount, debit or credit, to one balance of one account, with a value date and a booking date | a transaction record (`BankAcctTrnRec`): one line of an account's statement |
| **Booking** | entries that net to zero, booked together at one booking time: what a hook returns, and what C11 commits | none: IFX shows an account's side only |
| **Reversal** | a booking that undoes another, and names it | — |
| **Hold** | an amount authorised but not booked: it reduces the available balance, and is not an entry | — |
| **Balance address** | account, balance type, currency: what an entry moves | — |
| booking date, value date | when it was booked; when it counts | `PostedDt`, `EffDt` (to confirm against the spec's text) |

"Transaction" is not used in the model: it means one account's line in IFX, a
database transaction to the engine, and to a customer something that may only
be authorised. "Journal" is the instance's commit log (C12), never an
accounting term.

### Filters

Filters appear in three places, always declared, never ad hoc:

1. **What an extension or view covers:** `DepositTerms` exists where the
   product line is deposits; a view of accounts in arrears holds only those.
2. **What a hook fetches:** entries since the last capitalisation, balances
   of one type, as of the due (a hook's `#[requires]`).
3. **Who may see what:** see [Entitlements](#entitlements).

In a store designed per read, a filter on a column that is not a key is a
scan. So a filter that is read often is a view keyed by what it filters on:
"accounts in arrears by branch" is a view, not a query. A domain may define
filtered views over core types, but never a filter that changes what a core
entity means: the deposits model has a view of deposit accounts; it cannot
make `core::Account` mean one.

### Applying models

A model reaches a database as a **schema**, not as a crate:

```text
model crate ──build──► schema (canonical JSON, by digest) ──publish──► registry (OCI)
                                                        │
          tkone model plan deposits@3.2.0 ◄─────────────┘
            diff against the applied version
            → check the semver bump; classify each change
            → a plan: DDL per store, backfills, view rebuilds
          approve (maker and checker)
          tkone model apply → run the plan → record it applied
```

- **The schema.** The macros describe the model as data: entities, fields
  and their kinds of time, relations, extensions, views, keys and filters.
  `tkone model export` writes it as canonical JSON (RFC 8785), with each
  field's stable id from `model.lock`, and publishes it by digest, like an
  implementation ([the schema](contract-models.md#the-schema)). Applying
  needs no Rust toolchain, and a database is always traceable to the schema
  it was applied from.
- **Plan, then apply.** The plan shows each store's DDL, the backfills and
  the view rebuilds before anything runs. Applying is an administration
  [entitlement](#entitlements), approved by a second person.
- **The version is checked, not trusted.** A new field (optional or with a
  default), entity or view, or a deprecation, is a minor change; removing,
  retyping or re-keying is major, allowed only with an approved
  `--allow-breaking`. A plan whose version understates its changes is
  refused, and so is one that breaks the compatibility its kinds need
  ([compatibility](contract-models.md#3-compatibility-is-checked-per-kind)).
- **Additive changes apply directly.** A new view is backfilled by projecting
  the journals from their checkpoints, and serves reads once it has caught
  up.
- **Breaking changes expand, then contract.** The new shape is added beside
  the old; the engine writes both while both model versions are in use;
  existing data is backfilled; implementations move to definitions on the
  new version (C9); the old shape is dropped only when the registry shows no
  `live` or `deprecated` implementation depends on it (C7, C10).
- **Facts are never migrated in place.** A field new to a fact is absent on
  older ones (`Option` to a hook); a change of meaning is a new effective
  version, not an update. ScyllaDB cannot change a primary key: there, a
  re-keying is always a new table and a backfill.

The engine is **driven by applied schemas**, as a database is by its catalogue:
they tell it how actions map to tables and how to resolve a hook's
`requires`. Contracts are loaded at run time and models are applied without
redeploying it, so it cannot be compiled against them (C16). The generated
Rust types are for contract SDKs and applications. Extensions are the
exception: the ledger's meaning (a booking's entries net to zero, deliveries,
guarded balances) is compiled code in `tkone-ledger`, built into the engine
and required by the models that use it (C28).

---

## Time

Banking data has two times, and they often differ:

- **Valid time:** when something is true in the world: a deposit's value
  date, the day a rate takes effect.
- **System time:** when the bank recorded it: the booking.

On 3 June a branch books £1,000 with value 1 June. The accruals of 1 and 2
June were wrong in valid time, and right as known then. Both answers are
needed: the balance on 1 June as known now drives the correction; as known on
2 June, it answers audit and replay. So the model is **bitemporal by kind**
(C13):

| Kind | Declared as | Examples | Time | Changes by |
|---|---|---|---|---|
| **Facts** | `#[fact]` | bookings and their entries, operations, actions, hook calls | value time and booking time | never: appended; a correction is a reversal and a new fact |
| **Effective attributes** | `#[effective]` | status, rate, limits, roles, product binding | valid from and to, recorded at | a new version, future-dated or backdated |
| **Series** | `#[series]` | index fixings, FX, calendars | per date, versioned when restated | new points, restatements |
| **Fixed attributes** | plain fields | ids, `opened_on` | recorded at | never |
| **Derived views** | `#[view]` | balances, accrued interest, arrears | follow their sources | the engine |

Balances are not samples: they are a step function of entries, kept as
checkpoints (a closing balance per value date) and folded forward from the
nearest one.

**Authors never write temporal queries.** A hook's `requires` names what it
needs as of the due; the engine resolves it at one point and passes a plain
snapshot:

| Field | Meaning | In a catch-up of Friday's accrual on Monday |
|---|---|---|
| `due` | valid time: the business date | Friday 18:00 |
| `known_as_of` | system time: what the snapshot holds | commit N, Monday 09:02 |
| `lateness` | how late the run is | about 63 hours |

System time is the snapshot's, taken once, in commit order, and recorded with
the call; replay reads the recorded one, never the clock. Reading "as known
now" is deliberate: a fact booked on Saturday with value Friday is in Friday's
accrual first time; one with value Saturday is not. Reading "as it would have
been known on Friday" is for audit and reconstruction, not for running hooks.
Commit order, not wall clock, because engines on several nodes disagree on
the time, and Monday's call must see Friday's commit (D15).

### Backdated facts

A fact whose valid time is before results already computed makes them wrong.
The engine calls the instance's **`correction` hook** with the facts that
moved the past; it returns adjustments (an interest correction booking) as
new facts. History is never rewritten, and nothing re-runs (C14).

---

## Values: constants, code sets, parameters and reference data

Where a value lives follows from **who changes it, how often, and whether a
change needs approval** (C17):

| What | Examples | Lives in | Changed by | Through |
|---|---|---|---|---|
| **Constants** | an algorithm's constants, a rounding step fixed by the product's design | the definition's or implementation's code | developers | a new version |
| **Closed code sets** (the engine relies on them) | balance types, account statuses, booking sides | enums in the model, with their IFX and ISO 20022 codes | a model version | `model apply` |
| **Open code sets** (banks add to them) | product lines, fee types, branch codes | reference data | operations | the API, maker and checker |
| **Definition defaults** | a default notice period, a default rate | `#[parameter(levels(definition(default)))]` in the definition | product owners | registering a version |
| **Implementation values** | its rate card, calendar, accounting mapping | the signed manifest | implementers | publishing a version |
| **Instance overrides** | a negotiated rate, a waived fee | the database, effective | operations | the API, maker and checker |
| **Reference data** | indices, FX, rate cards, tariffs, the chart of accounts, calendars | the database, bitemporal | feeds, treasury, finance | the API, feeds, bulk loads; maker and checker |

If someone in the business could ever want to change a value, it is not a
constant: day counts, rates, ledger account numbers and grace periods are
parameters or reference data, however rarely they change. And there is no
global "settings" table: every value has a declared type, an owner, a level
and a kind of time.

### Reference data

Values shared across products are reference data, not parameters:

| | Examples | Owned by | Time |
|---|---|---|---|
| **Market** | SONIA, the base rate, FX, calendars | the market, a provider | series, restated in place |
| **Bank** | rate cards (treasury's tiered rates), fee tariffs | treasury, pricing | effective, versioned |
| **Accounting** | the chart of accounts (ledger accounts) | finance | effective |

Its shapes are declared in the core model (`#[series]`, `#[effective]`); its
values are data, set at run time through three channels in all:

1. **Code**, reviewed as a pull request and released by version: schemas,
   enums, defaults.
2. **Artifacts**, released by promotion (C7): implementation values in the
   signed manifest.
3. **The API and feeds**, effective-dated, approved by a second person and
   audited, with no redeploy: reference data and overrides.
   `tkone reference load rates.yaml` seeds from a reviewed file through the
   same API. Simulations use fixtures in memory, as `Calendar::fixed` does
   for calendars.

### Referring to reference data

Parameters refer to reference data **by a logical key**, resolved by the
engine as of the due, never by copying its values:

```rust
#[parameter(kind = "commercial", levels(implementation(default = "card:easy-saver"), instance()))]
const RATES: Ref<RateCard>;           // rate = RATES.tier(balance) + margin
#[parameter(kind = "operational", levels(implementation(required)))]
const ACCOUNTS: AccountingMapping;    // "interest_expense" → a ledger account
```

- **The snapshot says which version.** The resolved values a hook receives
  carry their versions, so replay and audit show "Friday's accrual used rate
  card version 12, tier 2".
- **Contracts name no ledger accounts.** A booking names logical accounts
  (`interest_expense`); the accounting mapping, per legal entity, turns them
  into ledger accounts. Finance can restructure the chart of accounts without
  a contract version.
- **A customer's `Account` and a `LedgerAccount` are different things.** An
  account's balances are addressed by account, balance type and currency;
  the mapping rolls them up into ledger accounts.

### Parameters

A **parameter** is a value a product defines, usually with a default, that
may be set at other levels within the product's limits: a rate, a margin, a
fee, an allowance, an early repayment charge schedule, a calendar. The word
is the one core banking uses for this; the alternatives each mean something
else here ("condition" is a predicate, "term" is a loan's duration, an
"attribute" is a model field, a "setting" sounds free to change).

```rust
#[parameter(kind = "commercial",
            levels(definition(default = "0.0075"),
                   instance(within = "default ± 0.005",
                            approve(when = "abs(new - old) > 0.0025", by = pricing_senior))))]
#[range(min = "0", max = "0.05")]
#[invariant("BASE.current + value <= CAP", reject = "above-cap")]
const MARGIN: Rate;

#[parameter(kind = "commercial", levels(definition(default = r#"{ bands: [
    { until_year: 1, rate: "0.02" }, { until_year: 2, rate: "0.01" } ] }"#)))]
const ERC: ErcSchedule;

#[parameter(kind = "operational", levels(implementation(default = "HalfUp")))]
const ROUNDING: Rounding;
```

**Any value type, built from model types.** A parameter's type may be a
scalar, a record, an enum with data, a bounded list, an `Option`, or a
`Ref<T>` to reference data, with the model's exact types (`Money`, `Rate`,
`Decimal<S>`, dates, durations, code sets). The model's rules apply, because
an override set in 2026 is read in 2050: no floats, bounded lists, stable
field ids, growth rather than breaking ([models](contract-models.md)). No
personal data: a parameter describes a product or an agreed term, not a
person.

```rust
/// Early repayment charge by year of the loan.
#[parameter_type]
#[invariant("size(bands) > 0")]
#[invariant("bands.all(i, b, i == 0 || b.until_year > bands[i-1].until_year)",
            reject = "bands-not-ascending")]
struct ErcSchedule { #[bounded(max = 10)] bands: Vec<ErcBand> }

#[parameter_type]
struct ErcBand {
    #[range(min = 1, max = 40)] until_year: u8,
    #[range(min = "0", max = "0.10")] rate: Rate,
}
```

**Declared where it is reused.** A type one product uses is declared in its
definition crate; one a domain's products share (`RateTiers`, `ErcSchedule`)
in the domain's model; values someone else owns for many products are
reference data, and the parameter is a `Ref<T>` saying which
([referring to reference data](#referring-to-reference-data)).

**Validated in five layers, each declared once:**

| Layer | Declared as | Checks | Example |
|---|---|---|---|
| Type | the Rust type | shape, exact types | a rate is not text |
| Field | `#[range]`, `#[bounded]`, `#[one_of]`, `#[len]` | each field, alone | `until_year` is 1 to 40 |
| Value | `#[invariant]` on the type | the value as a whole | bands ascending; the first tier starts at zero |
| Context | `#[invariant]` on the parameter | against other parameters and the instance | the margin keeps the rate under the cap; the currency is the account's |
| Level | `levels(…)` | who may set it, within what, with whose approval | an instance may vary ±0.50% from the default; a larger change needs a senior approver ([approval roles](#approval-roles)) |

Defaults are checked against all five when a definition or implementation
is registered, so a bad default never reaches `live`. An override, through
the API or a hook's `SetParameter`, is checked against all five when it is
set; an `approve` rule turns it into a maker-and-checker request. Nothing is
checked when a value is resolved for a call: it was checked when written.

**Levels.** A parameter lists the levels it may be set at; a value resolves
instance, then implementation, then definition, as of the due (C8).
`instance(required)` means every account must be given one when it opens
(a fixed deposit's rate). A parameter with no `instance` level cannot be
overridden per account.

**Kinds.** Values with different owners and legal weight are declared
apart:

| Kind | Examples | A change |
|---|---|---|
| `commercial` | rates, margins, fees, allowances, charges, the series a tracker follows | needs the approvals its levels declare, and emits `TermsChanged`, which statements and customer notices listen for |
| `operational` | rounding, calendars, accounting mappings | is an implementation or finance matter; no customer notice |

**Opinions:**

- **An override replaces the whole value.** One tier is never merged into a
  default's tiers: the value in force must be readable from one record. If
  parts change independently, they are two parameters.
- **Values are effective-dated whole.** "ERC schedule, version 3, from 1
  July" is one version; replay uses the version in force at the due.
- **Hooks get typed values:** a generated `Params` (`p.ERC.rate_for(year)`),
  never text to parse.
- **The schema drives the tools.** Parameter types are published in the
  definition's schema; the back-office form for a margin or a product's ERC
  schedule is generated from it, with the same validation.

**A parameter, or a facet field?** Both can be agreed per account. A **facet
field** is a fact of the account with no product default, set when it opens:
term end, repayment day, the property. A **parameter** is a value the
product defines, which an account may override within the product's limits:
margin, fees, allowance. If there is a product-level default, it is a
parameter.

---

## Entitlements

Who may do and see what, decided by the engine, never by contracts:

- **Operations:** whether this caller may invoke this operation on this
  instance: a teller may deposit; only a supervisor may waive a fee above a
  limit; a customer may withdraw from accounts where they hold a role.
- **Data:** which instances and fields a caller may read: rows filtered by
  legal entity, brand or jurisdiction; fields such as a party's tax id masked
  for most roles. Masking follows the model's classes (`personal`,
  `personal(special)`), not lists of fields
  ([personal data](contract-models.md#4-personal-data-is-classified-and-can-be-erased)).
- **Lifecycle:** who may register, approve and promote a contract version
  (C7), and who may set which parameters (a rate on one account).

Entitlements are checked before a call is resolved, and applied to every read
and API. A contract does not see who called, unless a definition makes an
attribute of the caller an input (a channel: branch or online). Entitlements
are attributes and policies over the model (party roles, the account's legal
entity), so they are declared next to it, not coded in hooks.

### Approval roles

Contracts and models need approvals: a margin change beyond a limit, a
restructured instalment, promotion to `live`, a breaking model change. But
who approves is the bank's organisation, which differs by bank, legal entity
and year. So the two are kept apart:

- **Code names abstract roles.** A definition, a model or the platform
  declares the roles its approvals need, with what each is for. Nothing in
  code names a person, a team or a directory group.
- **Each bank maps them.** An approval policy, per legal entity, says which
  groups hold each role and how many approvers it takes. It is data, set
  through the API with maker and checker, effective-dated like reference
  data; reorganising a team changes the policy, never a contract.

```rust
// in a definition: the roles its approvals name
approval_roles! {
    /// Approves a margin renegotiated beyond the instance limit.
    pricing_senior,
    /// Approves a restructured instalment.
    collections,
}

#[parameter(kind = "commercial",
            levels(instance(required,
                   approve(when = "abs(new - old) > 0.0025", by = pricing_senior))))]
const MARGIN: Rate;
```

```yaml
# policy/approvals-bank-uk.yaml: loaded through the API, maker and checker
legal_entity: bank-uk
effective_from: 2026-12-01
roles:
  offset-tracker.pricing_senior: { groups: [pricing-managers], approvers: 1 }
  personal-loan.collections:     { groups: [collections-leads], approvers: 1 }
  platform.product_owner:        { groups: [product-owners], approvers: 1 }
  platform.risk:                 { groups: [credit-risk], approvers: 2 }
  platform.model_owner:          { groups: [architecture], approvers: 1 }
```

| Declared by | Roles | Approve |
|---|---|---|
| the platform | `product_owner`, `risk`, `model_owner` | promotion to `live` (C7), `model apply`, `--allow-breaking` |
| a model | its own, if its actions need approval | a declared action with `approve(…)` |
| a definition | its own (`pricing_senior`, `collections`) | parameter changes, operations with `approve(…)` |

- **Checked at registration.** Every role an `approve` names must be
  declared, as `by = pricing_senior` is an identifier, not a string. And an
  implementation is not promoted to `live` in a legal entity whose policy
  leaves one of its roles unmapped: an approval nobody can give is found
  before an account needs it.
- **Never the maker.** The engine refuses an approval from the person who
  asked, whatever the policy says; `approvers: 2` means two people, neither
  the maker.
- **An approval is a request with a life:** pending, approved, rejected or
  expired. The change takes effect only once approved, from the date it asked
  for; if that date has passed by then, it is a backdated change and goes to
  the correction hook (C14).
- **Recorded with the change:** who asked, who approved, under which version
  of the policy, so audit reads one record.

---

## Definitions

A definition is a product's contract: what an implementation must do and may
return.

```rust
#[definition(name = "easy-saver", version = "2.1.0", model = "deposits@^3.1",
             implements = ["deposit-account@^1"])]   // Deposit, Withdraw, QuoteWithdrawal,
mod easy_saver {                                     //   their rejections and events
    #[parameter(kind = "commercial", levels(definition(default = "0.0325"), instance()))]
    #[range(min = "0", max = "0.15")]
    const RATE: Rate;
    #[parameter(kind = "operational", levels(implementation(default = "GBLO")))]
    const CALENDAR: CalendarCode;

    /// The product's own operation, beyond the interface.
    #[operation] struct SwitchProduct { to: ProductCode, from: NaiveDate }
    #[rejections(SwitchProduct)] enum SwitchRefused { NotEligible }
    #[event] struct BonusRateLost { from: NaiveDate }

    // Every operation, from the interface or the definition, is handled by one
    // hook named after it, which acts or refuses:
    //   deposit, withdraw, switch_product      -> Result<Outcome, Rejection>
    //   quote_withdrawal (a query)             -> Result<WithdrawalQuote, Rejection>
    // Lifecycle hooks are declared:
    #[hook(scheduled)]         fn end_of_day;         // tempo runs
    #[hook(credited)]          fn credited;           // a delivered credit landed (C22)
    #[hook(activation)]        fn open;
    #[hook(closure)]           fn close;
    #[hook(upgrade)]           fn upgrade;            // C9
    #[hook(correction, permits = [Book, Reverse])]
                               fn correct;            // C14: backdated facts

    permits! { Book, Reverse, Hold, Release, Transition(Active -> Dormant | Closed), ScheduleChange }

    schedules! {                                      // tempo definitions, below
        time_zone: "Europe/London",
        calendar: "GBLO",
        start: 2026-01-01,
        accrue:     "YY-MM-DD~SBT18:00:00",
        capitalise: "YY-MM-L~PBT18:00:00",
    }
}
```

Registering a definition persists its version, parameters and defaults, and
gives tempo its schedules as tempo definitions. It never runs. What each hook
receives and returns, the shape of every action, and events:
[hooks, outcomes and events](contract-hooks.md).

### Schedules

Schedules are declared in Rust, beside the rest of the definition. Tempo's
documents (YAML or JSON) are tempo's own format: the engine generates them;
nobody writes them for a product.

```rust
schedules! {
    time_zone: "Europe/London",      // every rule is read in it
    calendar: "GBLO",                // business days, for ~SB, ~NB, ~PB
    start: 2026-01-01,
    /// Interest, each business evening.
    accrue:     "YY-MM-DD~SBT18:00:00",
    /// Interest paid on the last business day of the month.
    capitalise: "YY-MM-L~PBT18:00:00",
}
```

- **One language for rules.** The rule strings are tkone-schedule's grammar,
  the same as everywhere else; a second, field-by-field way to say "the 15th,
  next business day, at nine" was rejected for tempo
  ([schedule documents](schedule-document.md#rejected-explicit-fields)) and is
  not reintroduced here. The macro parses each rule when the crate compiles,
  so a mistyped rule is a compile error, not a registration failure.
- **Each named rule is a tempo definition,** shared by every account on the
  product, under the definition's name and version (`easy-saver@2.1/accrue`).
  A rule that others derive from is declared with `basis`; a derived rule
  refers to it by name (`reminder: "{instalment}~2PT09:00:00"`), in one set
  (D13).
- **Generated for the code:** a `Rule` enum, so a run is tested with
  `run.is_for(Rule::Capitalise)` rather than a string; and a `Schedules`
  builder with one method per rule, so an account's selection cannot name a
  schedule the product does not have.

An account's own schedules are chosen by its contract, usually in `open`,
and returned as a `ScheduleChange` (D11):

```rust
let schedules = Schedules::new()
    .accrue(Select::between(o.opened_on, term_end))          // the rule as declared
    .capitalise(Select::from(o.opened_on))
    .instalment(Select::between(first_due, last_due)         // the account's own rule
        .rule(rule!("YY-MM-{day:02}~NBT09:00:00", day = o.repayment_day)));
```

`rule!` checks its template against the grammar at compile time; the values
are filled in at run time. Tempo checks at commit that every selection falls
on slots its definition produces, so a selection that cannot fire is refused,
and shows up in simulation first. Implementations may declare schedules of
their own with the same macro in their crate
([implementations](#implementations)).

### Operation schemas

A model's schema describes what is stored. Callers need the other half: what
they may send, what they get back when it is refused, and what events they
may receive. Those are declared as types too, and published as a schema the
same way (C25).

Operations shared by a family of products are an **interface**, so a channel
can withdraw from any deposit account without knowing its product:

```rust
#[interface(name = "deposit-account", version = "1.0.0", model = "deposits@^3.1")]
mod deposit_account {
    #[operation]
    struct Deposit {
        amount: Money,
        channel: Channel,
        #[personal(subject = "payer")] reference: Option<Text>,
    }

    /// Returns the state after the change, from with-instance views.
    #[operation(returns = { available: AccountState.available, holds: AccountState.holds })]
    #[accepts_facets]                    // adopters may attach typed input (extending-the-base.md)
    struct Withdraw { amount: Money, channel: Channel }

    /// An answer computed by the product, not stored: no actions.
    #[query(returns = WithdrawalQuote)]
    struct QuoteWithdrawal { amount: Money, on: NaiveDate }

    /// Why a withdrawal is refused: declared, so a channel can act on it.
    #[rejections(Withdraw)]
    enum WithdrawRefused {
        InsufficientFunds { available: Money },
        NoticeRequired { earliest: NaiveDate },
        Blocked,
    }

    #[event]
    struct InterestCapitalised { amount: Money, period: DateRange }
}

// easy-saver@2.1.0, above, implements it.
```

One declaration, four outputs:

| Output | For | Example |
|---|---|---|
| The schema (canonical JSON, by digest) | the registry, `plan` checks, audit | `deposit-account@sha256:…` |
| WIT | implementations | `withdraw: func(op: withdraw, input: withdraw-input, params: params) -> result<outcome, withdraw-refused>` |
| OpenAPI | the engine server's API | `POST /accounts/{account}/operations/withdraw` |
| Client types | channels, in Rust, TypeScript, Python | `client.withdraw(account, { amount, channel })` |

```text
POST /accounts/iban:GB33BUKB20201555555555/operations/withdraw
Tkone-Interface: deposit-account@^1
Idempotency-Key: 7f9c2e…
{ "amount": { "minor": "50000", "currency": "GBP" }, "channel": "online" }

422 Unprocessable
{ "rejected": "notice-required", "earliest": "2026-12-16" }

200 OK
{ "instance_version": 1843, "bookings": ["bkg_01JD…"],
  "result": { "available": { "minor": "120000", "currency": "GBP" }, "holds": [] } }
```

```text
$ tkone contract describe deposit-account@1
deposit-account 1.0.0                       model deposits@^3.1
  operations   deposit, withdraw
  queries      quote-withdrawal
  rejections   withdraw: insufficient-funds, notice-required, blocked
  events       interest-capitalised
  implemented  easy-saver@2.1.0 (live, default), notice-90@1.3.0 (live)
```

- **The model's rules apply to payloads.** Fields have stable ids (a
  `definition.lock`), grow rather than break, and are classified: a payer's
  reference is personal. Payloads are checked backward compatible, so a
  caller built for 1.0 is accepted by every 1.x
  ([models](contract-models.md)).
- **The caller names the interface,** and the engine checks the instance's
  definition implements a matching version. A withdrawal sent to a loan is a
  `409` naming what the account is, not a hook's surprise.
- **Rejections are typed, and open to growth.** A new reason is a minor
  version; generated clients carry an `Unknown { code }` case, so an older
  channel shows a generic refusal instead of failing.
- **Events are typed.** A hook emits only events the interface or the
  definition declares; the engine adds an event for every committed action
  ([events](contract-hooks.md#events)).
- **Results are declared, not computed by hooks:** a receipt (the instance's
  new version and the bookings made) and the operation's `returns`, a
  selection of `with-instance` views. Answers that are computed and not
  stored are queries ([results](contract-hooks.md#operation-results),
  [queries](contract-hooks.md#queries)).

---

## Implementations

An implementation is a WASM component implementing one definition version's
WIT world, published to an OCI registry, signed, and registered by digest.
It is built with `tkone-contract-sdk` for `wasm32-wasip2`, or in another
language that targets components (componentize-py for Python).

Everything the engine needs to know about it is declared on the code:

```rust
#[implementation(of = "easy-saver@2.1.0", values(CALENDAR = "GBLO"))]   // its own values
impl easy_saver::Contract for Saver {
    #[requires(view(AccountState),
               balances(as_of = due),
               entries(since = last(Rule::Capitalise), until = due, max = 2000))]
    fn end_of_day(run: Run, input: input::EndOfDay, p: Params) -> Outcome { /* … */ }

    #[requires(view(AccountState))]
    fn withdraw(op: Withdraw, input: input::Withdraw, p: Params)
        -> Result<Outcome, WithdrawRefused> { /* … */ }
}

schedules! {                                  // its own schedules, beside the definition's
    time_zone: "Europe/London", calendar: "GBLO", start: 2026-01-01,
    bonus_review: "YY-MM-L~PBT09:00:00",
}
```

| Input | Fetches |
|---|---|
| `view(V)` | a view of this instance, as of its last commit |
| `balances(as_of = due)` | its balances at a time |
| `entries(since, until, max)` | its entries in a window, bounded (C21) |
| `series(PARAM, as_of)`, `effective(T, active_at)` | reference data, effective records |
| `account(from, read)` | a few fields of one other account, named by the payload or the terms; eventual |
| `linked(via, read, as_of, settled, max)` | other accounts through a relation; `settled` waits until they have passed the time (C5, C29) |
| `personal(Field)` | a personal field, withheld unless named (C19) |

`#[requires]` is what makes hooks fast: the engine fetches exactly that, in
bulk, before the call, and the hook does no I/O at all (C3). Each hook's
input type is generated from it, so the code cannot read what it did not
declare. It is also the only way a hook reads another instance
([offset mortgage example](example-advanced-offset-mortgage.md#5-implementation)).

`cargo component build` writes the **manifest** from these attributes and
`schedules!`: what it implements, its values, its schedules, each hook's
inputs. It travels with the component, is signed with it, and is what the
engine reads at registration without running any code. Nobody edits it.

---

## The invocation pipeline

One operation on one instance:

1. **Entitle and resolve.** The caller's [entitlements](#entitlements) for
   this operation and instance; then the instance's binding: definition
   version, implementation digest, overrides. A deprecated implementation
   still runs for bound instances; an archived one never does (C7).
2. **Parameters,** resolved as of the due (C8): instance override, else
   implementation value, else definition default, with the reference data
   they refer to.
3. **Fetch** the hook's `requires` as of the due, in a snapshot taken at the
   instance's last commit (C5, C13). For a bulk call, fetches are batched.
4. **Invoke** in a fresh component instance: no I/O, the clock passed in,
   with fuel and memory limits. With wasmtime's pooling allocator,
   instantiation is microseconds; the cost is in moving the inputs, which is
   why they are prepared and small.
5. **Validate** the outcome: its actions permitted by the definition and the
   hook, valid against the model's invariants (a booking's entries net to
   zero; a transition is allowed from the current state); its events
   declared ([actions](contract-hooks.md#actions)).
6. **Commit** per instance: one conditional append to the instance's
   journal, the actions with "only if the instance is still at version n"
   (C12). The `with-instance` views they change, the outbox (for `eventual`
   views and events) and tempo's schedule changes go with it. Entries for
   other accounts (credits, and entries to unguarded accounts) are delivered
   to their inboxes, not committed in their journals (C22).
7. **Project** `eventual` views from the outbox, asynchronously
   ([outbox and inbox](outbox-inbox-design.md)).

An operation's hook may also **reject** (a withdrawal refused): nothing is
committed, and the typed rejection is the result. A **query** stops after
step 4: its value is the result, and nothing is validated or committed.

### Bulk

A bulk call is many invocations in one request, each committed on its own:
the result is per item, never all or nothing. Items for different instances
run in parallel; items for the same instance run in order, each seeing the
previous one's commit (C6).

---

## Actions

A hook returns an **outcome**: the actions it wants taken and the events it
wants told, or a typed rejection.

```rust
pub struct Outcome<A = Action, E = Event> { pub actions: Vec<A>, pub events: Vec<E> }
```

The engine must understand every action, but it does not own them. They come
in three tiers:

| Tier | What | Declared in | Banking's |
|---|---|---|---|
| **Kernel primitives** | `Append`, `Set`, `Create`/`End`, `Transition`, `Deliver`, `ScheduleChange`: what storage can do to any model's kinds | the engine: closed, small | — |
| **Declared actions** | a primitive on a model's type, with invariants (CEL) checked at commit | model crates: open to any domain | `Hold`/`Release`, `Transition`, `SetParameter` |
| **Extensions** | actions whose meaning needs code; they plan writes in primitives, the engine writes | Rust crates built into the engine, required by models | the ledger: `Book`, `Reverse` |

A definition narrows the actions its models declare (`permits!`), and may
narrow them per hook; the
SDK's generated types hold only what is permitted, so anything else does not
compile, and the engine checks again at commit. A contract can never return
"write this row": if it could, the engine could not keep the views true.
Events are declared types, emitted with the actions; the engine adds an
event for every committed action. There are no free-form attributes and no
opaque result bytes. The types, the checks per action, and the event
envelope: [hooks, outcomes and events](contract-hooks.md).

---

## Tempo

| Contract side | Tempo side |
|---|---|
| A definition's or implementation's `schedules!` | tempo **definitions** (shared, product-wide), generated |
| Opening an account | its **instances**: `Schedules` selections with the account's dates, returned by `open` (D11) |
| A due run | an engine call of the `scheduled` hook, with the slot's sources: `run.is_for(Rule::Capitalise)` |
| A missed run | the instance's catch-up policy, `stale_after` (D15) |
| `ScheduleChange`, an upgrade | the next version of the account's set (D12, D13) |
| Parameters as of the due | the run acts on its due, not on now (D15) |

Several rules due at once are one run (D14): end of day accrues, then
capitalises at the month end, in one hook.

---

## Storage

The engine relies on **single-partition atomicity only**: a commit is one
conditional append to the instance's journal. Everything else is a
projection of journals. Postgres does the append with a version check in a
transaction; ScyllaDB with a lightweight transaction (`IF version = n`) on
one partition. The framework is opinionated about the shape, so the store is
a deployment choice:

- **The instance is the partition key.** Its journal, facts and effective
  attributes are clustered by valid time and recorded time: an as-of read is
  a slice of one partition.
- **Append only.** Facts are never updated; effective attributes gain
  versions.
- **A table per read**, no joins on a hook's path. Filters that are read
  often are views keyed by them.
- **Views declare their consistency.** `with-instance` views are written by
  the engine with the commit: in Postgres in the same transaction; in
  ScyllaDB right after the append, idempotently, and repaired from the
  journal if that write fails. `eventual` views are projected from the
  outbox.

Database materialized views are a storage detail the model never sees. A
ScyllaDB materialized view re-keys the same rows with a limited filter: it
fits a pure re-keying view ("accounts by party"), and with
`synchronous_updates` it can serve a `with-instance` one. It is not a
transaction (the base row can be written when the view's write fails), it
costs availability (a write fails when view replicas cannot be reached), and
it cannot aggregate or join. Balances, accruals and arrears are folds, so the
engine computes them on any store.

The ledger mostly fits that shape too. A booking commits in its **home**, the
journal of the instance whose operation produced it, where its guarded
debits are checked; its credits and entries to unguarded accounts are
**delivered** to inboxes, insert only, because nothing they would be checked
against can be wrong. So a transfer is one partition's commit, and a busy
account (fee income, a merchant) serialises nothing ([hot
accounts](hot-accounts.md), C22). On Postgres delivery is in the same
transaction; on ScyllaDB it is projected from the outbox.

What is left is a booking that **debits two guarded accounts** at once. Postgres
does it in one transaction; ScyllaDB has no transaction across partitions, so
there it would be a saga (steps to each journal through the outbox,
compensation). It is rare, and its cost is part of why Postgres comes first
(C12).

Partitions are bounded: an instance's journal is bucketed by period, so no
partition grows for the life of a mortgage. Every record names the schemas
and the code it was written under (C23).

---

## Decisions

### C1. Contracts return actions; the engine writes

Hooks never touch the database. The engine is the only writer, so every
write is checked against the definition and the model, every view stays
derivable, and a hook is a pure function that can be replayed. The actions
a hook may return are declared by its models, on the engine's primitives
(C28).

### C2. Implementations are pinned by digest

Instances bind to an OCI digest, never a tag: a tag can be moved to other
code, which would make an account's history unexplainable. Signatures
(sigstore/cosign) are verified when the implementation is registered and when
it is loaded.

### C3. Hooks are deterministic

No I/O, no clock, no randomness: time is passed in, data is fetched per the
manifest. The engine records a hash of each call's inputs. Same component,
same inputs, same actions: that gives audit, replay and simulation.

### C4. Views are derived, never written

Views are projections of entities and entries, maintained by the engine
from actions. Each view declares its consistency: `with-instance` views are
written in the commit; `eventual` views are projected from the outbox.

### C5. A hook reads its own instance as of its last commit

What a hook reads about its own account is `with-instance`: committed with the
last action, so two operations cannot both see the same balance. Data about
other instances (an offset savings balance for a mortgage's interest) is a
declared, read-only input from an `eventual` view, as of a time the hook
states; `settled` makes the engine wait until those instances' journals and
inboxes have passed that time. Every such read is recorded, so a late fact
on what was read reaches the reader's correction hook (C29).

### C6. One writer per instance

Operations on one instance are serialised: by partitioning on the instance,
with the instance's version checked at commit (optimistic, retried). Different
instances never wait for each other. "Every invocation is isolated" holds
between instances, not within one.

### C7. Implementation versions: four states and a default

| State | New bindings | Calls | Notes |
|---|---|---|---|
| `draft` | test and simulation instances only | there | may be re-published |
| `live` | yes | yes | immutable and signed; may be the default |
| `deprecated` | no | for instances already bound | runs until they move or close |
| `archived` | no | no | only once no instance is bound; history stays readable |

**Default** is not a state but a pointer, per definition: the `live` version
new instances get. As a state it would allow "default and deprecated".
Moves are forward only: `draft → live → deprecated → archived`.

### C8. Parameters are effective-dated and resolved per call

A value is resolved as instance override, else implementation value, else
definition default, as of the call's due. A rate change on 1 July is a new
value effective from 1 July: a catch-up accrual for 30 June uses June's rate.
Values shared across products (rate cards, indices, the chart of accounts)
are reference data, which parameters refer to. Each parameter is a typed value, validated
when it is set, never when it is read (C31).

### C9. Moving an instance to a new version is an upgrade

Rebinding an account is a new binding version. The new implementation's
`upgrade` hook converts the instance's state, and its schedules become the
next version of the account's tempo set (D12). Upgrades are explicit, per
instance or in bulk, never implied by promoting a new default.

### C10. Versions are checked at registration

An implementation targets exactly one definition version; a definition, a
semver range of one model. The engine refuses at registration what does not
fit (a model version not applied, a parameter missing), never at call time.

### C11. The ledger is the core's, as an extension

Money moves only as bookings: entries that net to zero. Balances are views of
entries. An operation across accounts (a transfer) is one booking, committed
in its home instance with the other accounts' entries delivered (C22); it
goes through the ledger, not through hooks writing to each other. A booking
that debits two guarded accounts is the one exception to C6's one instance
per commit. The ledger is not in the engine: it is the first extension
(C28), required by the core model, so the engine stays free of any domain.

### C12. A commit is a journal append; Postgres first

A commit is one conditional append to the instance's journal; views are
projections ([Storage](#storage)). That needs atomicity in one partition
only, which Postgres and ScyllaDB both have, so storage is behind a trait.
Postgres comes first, with tables designed as for ScyllaDB: transactions make
C5, C6 and C11 simple while the product takes shape. A ScyllaDB store is added
when load says so, and if the shape has held, no model or contract changes.

### C13. The model is bitemporal by kind

Facts carry value and booking time; effective attributes, valid and recorded
time; series, a point per date; fixed attributes, recorded time only
([Time](#time)). A hook reads valid time at its due and system time at a
snapshot taken once, in commit order, and recorded. Authors never write a
temporal query.

### C14. Backdated facts go to a correction hook

A fact that changes results already computed calls the instance's
`correction` hook, which returns adjustments as new facts. History is not
rewritten and hooks are not re-run: re-running would post a second set of
accruals to net against the first.

### C15. The core is small, and extended, never redefined

Party, account, ledger and reference data are the core. Domain models extend
its entities with facets and define their own views over it, filtered as they
need; they never change what a core entity means. Names are full English
words, with the vocabulary and codes of IFX and ISO 20022.

### C16. Models are applied as published schemas; the engine is driven by them

A model crate exports a schema, published by digest; `tkone model plan` and
`apply` take a database from one applied version to the next, with the
semver bump checked and breaking changes expanded, then contracted once no
implementation depends on the old shape ([Applying
models](#applying-models)). The engine reads the applied schemas, so models
are applied and contracts loaded without redeploying it.

### C17. Values live where their owner changes them

Constants in code; closed code sets in the model; defaults in definitions;
implementation values in the signed manifest; overrides and reference data in
the database, effective-dated, set through the API with maker and checker
([Values](#values-constants-code-sets-parameters-and-reference-data)).
Contracts refer to reference data and ledger accounts by logical key, resolved
as of the due, and each resolved value carries its version.

### C18. Fields have stable ids; models grow, compatibility is checked per kind

Every entity, field, relation and view has an id, assigned by tooling and
kept in a committed `model.lock`, so a rename is free and an id is never
reused. Models add and deprecate; a field is never removed or redefined in
place, and a breaking change is an approved exception. `model plan` checks
the compatibility each kind needs: facts readable by every version, views
rebuilt instead. Each component is given its inputs in the model version it
was built for, by field id
([models 1–3](contract-models.md#1-fields-have-stable-identities)).

### C19. Personal data is classified when declared, and erased by key

Fields are classed (`internal` by default, `personal`, `personal(special)`,
and others), with the party they belong to. Personal fields are encrypted
with a key per party; erasure deletes the key, everywhere at once, while
bookings stay, pseudonymous. Retention and legal holds are policies the
engine runs; free text is never left unclassified; hooks receive personal
fields only when they declare them
([models 4](contract-models.md#4-personal-data-is-classified-and-can-be-erased)).

### C20. Numbers are exact, and rounding is explicit

No floats in models, definitions or the SDK. Money is minor units and a
currency; rates and other values are fixed-point decimals. Arithmetic that
can lose precision returns `Unrounded<T>`, rounded by a mode the definition
declares ([models 5](contract-models.md#5-numbers-are-exact)).

### C21. Every collection a hook reads is bounded

Relations and collections declare a `max`; a `requires` without a bound is
refused at registration; a call whose inputs exceed their bound goes to an
operator. Folding many rows is a view's job
([models 6](contract-models.md#6-every-collection-is-bounded)).

### C22. Only guarded writes are serialised

A booking commits in its home instance, where guarded debits are checked;
credits, and entries to unguarded accounts, are delivered to insert-only,
sharded inboxes. A check can miss only in-flight credits, so it never passes
wrongly. Busy accounts serialise nothing, and transfers are single-partition
commits. A product that must be able to refuse money guards credits too:
they land pending, its `credited` hook accepts or refuses them, and a
refusal is a return to the sender, still without a commit across partitions
([hot accounts](hot-accounts.md), [guarded
credits](hot-accounts.md#guarded-credits)).

### C23. Every record names its schema

Journal records carry the digests of the schemas they were written under, the
implementation, the engine version and the input hash. Readers decode by
field id under that schema and lift values to the version they read at
([models 8](contract-models.md#8-every-record-names-its-schema)).

### C24. Identities are typed and opaque; business identifiers are not keys

Keys are engine-generated, typed per entity, and never shown to customers.
An object carries a set of identifiers (account number, IBAN, PAN, passport,
LEI, a legacy system's id), effective-dated. Their schemes are reference
data: what each applies to, its format and check, its uniqueness scope, how
many an object may hold, who issues it. Definitions say which their product
issues and needs; the engine allocates bank-issued ones. Any identifier can
stand in for a key in the API. Operations take idempotency keys. Parties
merge by an effective redirect; nothing that refers to them is rewritten
([models 9](contract-models.md#9-identities-are-typed-and-survive-merges)).

### C25. Operations, rejections and events are declared types, published as schemas

What callers send, why they may be refused, and what they are told are typed
in interfaces (shared by a family of products) and definitions, and
published by digest like a model's schema. WIT, OpenAPI and client types are
generated from them. Callers name the interface they expect; payloads follow
the model's rules: stable ids, growth, classes
([operation schemas](#operation-schemas)).

### C26. Views are declared in four forms, not coded

Re-key, latest, fold (mergeable aggregations over facts) and compose (with
arithmetic formulas). The engine derives and maintains every view from them;
a derivation is part of the schema, and a changed one is a new view version
([views](contract-models.md#views-four-forms-no-code)). A projector component
is a later, exceptional escape hatch.

### C27. A hook returns an outcome of declared types; results come from views

An outcome is the actions (declared by its models, narrowed per definition
and per hook, checked again at commit) and the events (declared) it wants, or
a typed rejection. The engine emits an event for every committed action. There
are no free-form attributes and no opaque result bytes: an operation's
result is a declared selection of `with-instance` views, and computed answers
are queries, which return values and never actions
([hooks, outcomes and events](contract-hooks.md)).

### C30. Schedules and inputs are declared in Rust; documents are generated

A definition declares its schedules with `schedules!`, in tkone-schedule's
rule grammar, parsed at compile time; registration generates the tempo
definitions. Accounts select from them with a generated `Schedules` builder,
returned as a `ScheduleChange`; runs are tested against a generated `Rule`
enum. An implementation declares its values with `#[implementation]` and
each hook's inputs with `#[requires]`, from which its input types are
generated; the build writes the manifest the engine reads. Nobody writes
tempo's documents or a manifest for a product ([schedules](#schedules),
[implementations](#implementations)).

### C31. Parameters are typed values with declared validation, levels and a kind

A parameter may be any value type built from model types (records, enums,
bounded lists, references), under the model's rules. It is validated in five
declared layers (type, field, value, context, level) when a default is
registered or an override is set. Its levels say where it may be set, within
what and with whose approval; its kind (`commercial` or `operational`)
decides whether a change notifies customers. An override replaces the whole
value, effective-dated ([parameters](#parameters)).

### C32. Approvals name abstract roles; each bank maps them

Definitions, models and the platform declare the approval roles they need,
by name and purpose; `approve(by = …)` refers to one. Each legal entity's
approval policy, effective-dated data set with maker and checker, maps roles
to groups and approver counts. Registration checks every named role is
declared, and promotion to `live` checks the legal entity maps them all. The
engine never accepts the maker as an approver, and records the request, the
approvers and the policy version with the change
([approval roles](#approval-roles)).

### C28. Actions are declared by models on kernel primitives; extensions are rare

The engine's own set is a few primitives that mirror storage: append a fact,
set an effective attribute, create or end an effective entity, transition a
declared state machine, deliver to another instance's inbox, change a tempo
set. Models declare their domain's actions on them, with invariants in CEL
over one instance's snapshot. Meanings that need code are extensions: Rust
crates built into the engine, which plan writes in primitives and never
write themselves; a model names the extensions it requires, and registration
refuses an engine without them. Banking's v1 needs one, the ledger. If most
models need an extension, a primitive is missing
([actions](contract-hooks.md#actions)).

### C29. Instances are drawn around invariants; across them, five tools

An invariant reads one instance, so the instance is the consistency
boundary: draw it so the rules that must hold at every commit fall inside it
(a mortgage's fixed and tracker parts are one instance). Rules that involve
other instances use, in this order: **declared inputs**, settled if they
must be complete; **dependent corrections**, which the engine derives from
recorded reads; **claims** for uniqueness across instances (`unique` on a
relation, as for identifiers); **facilities**, an instance of its own for a
shared limit, which guards it; and only then an extension with a commit
across partitions. Product logic is not limited by this: it is hook code,
over whatever inputs its `#[requires]` declares
([offset mortgage example](example-advanced-offset-mortgage.md#across-instances-what-this-product-needs-and-what-it-does-not)).

### C33 to C39. Extending the base

How adopters extend the models, interfaces and products the platform ships
is decided in [extending the base](extending-the-base.md#decisions), with an
example for each:

- **C33.** Namespaces have owners; adopters depend on the base, never the
  reverse; forks are explicit.
- **C34.** Adopters extend data with facets, open code sets and profiles.
- **C35.** New operations and events come in the adopter's own interfaces.
- **C36.** Operation facets carry adopter input on base operations, and
  change nothing in base code.
- **C37.** Behaviour is reused by library, never overridden.
- **C38.** The base declares its extension points; what is undeclared is
  closed.
- **C39.** What runs is a distribution, upgraded and checked as one.

---

## Crates and apps

| Crate or app | Role |
|---|---|
| `tkone-model`, `tkone-model-macros` | `#[model]`, `#[entity]`, `#[extends]`, `#[fact]`, `#[effective]`, `#[series]`, `#[view]`, `#[personal]`, `#[identifiers]`; `model.lock`; the schema; compatibility checks; DDL diff; WIT generation |
| `tkone-core` | The core model: party, account, ledger, reference data; the vocabulary |
| `tkone-store` | The storage trait, journals and projections; Postgres first |
| `tkone-contract` | Interfaces and definitions: operations, queries, rejections, events, hooks, parameters, `schedules!`; `Outcome`, `Action`, `permits!`; their schemas, WIT worlds and OpenAPI; tempo definitions generated from `schedules!` |
| `tkone-contract-sdk` | Writing an implementation in Rust for `wasm32-wasip2`: `#[implementation]`, `#[requires]`, generated input types; the manifest, generated at build |
| `tkone-ledger` | The first extension (C28): `ledger.book`, `ledger.reverse`; homes, inboxes, guarded credits and returns, checkpoints, reconciliation (C22); `Money`, `Rate`, rounding |
| `tkone-engine` | Kernel primitives and the `Extension` trait; registries, OCI loading and verification, the pipeline, CEL invariants, commits, projections |
| `tkone-products-*` | The base's products as crates of pure functions, beside their components, for adopters to reuse (C37) |
| `tkone-tempo` | Scheduling ([schedule documents](schedule-document.md)) |
| engine server (app) | APIs: registries, lifecycle (C7), bindings, operations, bulk |
| `tkone` CLI (app) | `model export`, `check`, `plan`, `apply`; `journal show`; `party merge`; `contract describe`, `publish`, `promote`, `deprecate`, `archive`; `reference load`; `simulate` |

Existing crates to build on: wasmtime (components, pooling, fuel), wit-bindgen
and cargo-component, `oci-client` and wasm-pkg-tools, sigstore-rs, sqlx; CEL
(`cel-interpreter`) if guards in definitions need expressions.

## Simulation

An engine over an in-memory store, stepped by a [timeline](timeline.md) of the
instance's schedules: open an account, deposit, run ten years of end-of-day
hooks, and assert on bookings. It runs the real component (C3), so it is how
a product is tested before it goes `live`, and how a change is shown to
differ from the version before.

## Open questions

- **Who writes contracts?** Domain experts rarely write Rust. WIT allows
  Python through componentize-py, but the authoring experience (SDK, local
  simulation, errors) is a product of its own.
- **WIT and model evolution.** Answered in part: each component is given
  inputs in the version it was built for (C18). Open: how a component's WIT
  world names that version, and how long the engine keeps old shapes.
- **Limits.** How much a hook may fetch is bounded (C21); how long it may
  run, and the fuel it gets, is not yet set.
- **Erasure and retention, per jurisdiction.** Whether pseudonymous bookings
  satisfy erasure where the bank operates (C19) is for its lawyers; the
  model only makes the answer a policy.
- **How long end of day waits for deliveries** before a run is stale (C22).
- **Hooks across instances** beyond the tools of C29 (inputs, dependent
  corrections, claims, facilities): wait for a real product that needs them.
- **How long a settled read waits** for another instance's journal and inbox
  to pass the due, and what a run does when one is behind (C5, C29).
- **Corrections far back.** A fact backdated by months: how much the
  correction hook is given, and whether a limit sends it to an operator.
- **Evaluating entitlement policies:** built-in rules over roles and the
  model's attributes, or an external policy engine called by the engine.
  Approval roles (C32) are mapped the same way in either case.

## First slice

Before any framework: an easy-access savings account (deposit, withdraw,
end-of-day accrue, month-end capitalise) as one WASM implementation, on an
engine over Postgres, with tempo's schedules and a simulation test. Hand-write
its model and definition; extract `#[model]` and `#[definition]` when a second
product (a loan) shows what they share.
