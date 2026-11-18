# Developer experience — Design decisions

Status: **architecture only**, like the [contract engine](contract-engine.md)
it belongs to. The code sketches are illustrative, not APIs.

Models, interfaces, definitions and implementations are Rust crates, written
with the platform's macros. Whether a developer (or an AI agent working
for one) is productive in them depends less on the macros' power than on
what the editor understands: completion, go to definition, rename, hover,
and errors underlined where the mistake is.

This document decides how the syntax and tools are shaped for that. Each
decision is shown before and after, then stated as a decision (C42 to C46)
at the end.

---

## Who it is for, and the balance

Most code here may be written by AI agents, reviewed by people. That raises
the bar rather than lowering it: an agent learns what is possible from the
same places a person does (completion, types, docs on hover) and corrects
itself from the same signals (a compiler error that names the mistake and
the alternatives). A mistake the compiler catches costs seconds; one caught
at registration costs a pipeline run; one caught in production costs an
incident.

So the balance is:

- **Spend on what the editor can use.** Macros that generate real types,
  carry the author's spans into their output and check references through
  the compiler are worth their cost, even when that cost is visible.
- **Do not spend on what the editor cannot use.** Exporting schemas,
  checking compatibility against `model.lock`, generating WIT and running
  simulations happen in `cargo tkone` and CI, not in a macro run on every
  keystroke.
- **Measure before trading.** No editor capability is given up to save
  build time on a guess; the cost is measured first ([where the cost
  goes](#where-the-cost-goes)).

## What a developer writes

A product is a Cargo workspace member like any other:

```toml
# products/fixed-12m/Cargo.toml
[package]
name    = "fixed-12m"
version = "1.0.0"                       # the definition's version: nowhere else

[dependencies]
tkone-contract     = "1"                # #[definition], #[parameter], schedules!, …
tkone-ledger       = "1"                # #[ledger], booking!, Money
term-deposits      = "1.0"              # the model it uses: a crate, resolved by Cargo
```

```rust
// products/fixed-12m/src/lib.rs
use tkone_contract::prelude::*;
use term_deposits::{self, TermDepositBalance};

#[definition(name = "fixed-12m", model = term_deposits)]
#[ledger(guard = DebitsAndCredits, credits_to = TermDepositBalance::Principal)]
mod fixed_12m {
    #[parameter]
    const RATE: Parameter<Rate> = Parameter::commercial()
        .instance(Instance::required())                       // fixed when opened
        .range(rate!(0), rate!(0.10));
    // …
}
```

Everything after `#[` is completed by the editor, every name resolves, and
every mistake is a compiler error at the mistake.

---

## 1. References are paths, not strings

A string means nothing to an editor: no completion, no go to definition, no
rename, and a typo surfaces at registration. Anything that **refers** to
something else is a Rust path; strings remain only for **names** (a
product's or model's identity, which outlives any crate) and for the
[embedded languages](#5-embedded-languages-are-marked-and-served).

| Before | After | Refers to |
|---|---|---|
| `model = "deposits@^3.1"` | `model = deposits` | the model crate, a Cargo dependency |
| `implements = ["deposit-account@^1"]` | `implements = [deposit_account]` | the interface's module |
| `#[implementation(of = "easy-saver@2.1.0")]` | `#[implementation(of = easy_saver)]` | the definition's module |
| `extensions = ["ledger@^1"]` | `extensions = [ledger]` | the extension crate |
| `guard = "debits-and-credits"` | `guard = DebitsAndCredits` | `tkone_ledger::Guard` |
| `credits_to = "Principal"` | `credits_to = TermDepositBalance::Principal` | the model's balance type |
| `kind = "commercial"` | `Parameter::commercial()` | a constructor |
| `#[action(set = "SetPayTo")]` | `#[action(set = SetPayTo)]` | the generated action |
| `#[action(extension = "ledger.book")]` | `#[action(extension = ledger::book)]` | the extension's action |
| `initial = "Current"` | `initial = Current` | the state machine's state |
| `role = "Holder"` | `role = Role::Holder` | the core's role |
| `default = "HalfUp"` | `.implementation(Rounding::HalfUp)` | the enum variant |

**How it works.** A macro cannot see other crates' items, so it does not
check these itself. It copies each path into its output, with the author's
span, where the compiler must resolve it:

```rust
// what #[ledger(credits_to = TermDepositBalance::Principal)] emits, among the rest
const _: TermDepositBalance = TermDepositBalance::Principal;
```

rust-analyzer resolves the path in the expansion and maps it back to the
source, so the author gets completion after `TermDepositBalance::`, go to
definition, rename across crates, and the compiler's own error for a typo:

```text
error[E0599]: no variant named `Principl` found for enum `TermDepositBalance`
  --> products/fixed-12m/src/lib.rs:6:59
   |
6  | #[ledger(guard = DebitsAndCredits, credits_to = TermDepositBalance::Principl)]
   |                                                                   ^^^^^^^^ help: there is a variant with a similar name: `Principal`
```

## 2. Cargo owns versions and dependencies

`model = "deposits@^3.1"` repeated what Cargo already does, and could
disagree with it. Model, interface, extension and product crates are
ordinary dependencies:

- **A crate's version is its artifact's version.** A definition's version is
  its `package.version`; a model's is its crate's. The macros read
  `CARGO_PKG_VERSION`; an attribute never repeats it.
- **Ranges are Cargo's.** `term-deposits = "1.0"` is the definition's model
  range; `cargo update` moves it within the range; `Cargo.lock` pins what
  was built, and its digests go into the signed manifest.
- **The distribution lock is mostly `Cargo.lock`.** What
  [extending the base](extending-the-base.md#upgrading-the-base) calls a
  distribution is the base's crates and the adopter's, resolved together by
  Cargo; `tkone` adds only the schema digests Cargo does not know.
- **Registries are Cargo registries.** The platform's crates come from a
  registry the bank mirrors; an adopter publishes to its own. Signing and
  supply-chain checks for crates are the ones the bank already runs.

## 3. References across crates are checked by the compiler

Some references are deeper than a path: a field of a view, a view that must
be `with-instance`, a fact type with a quantity. The macro turns each into
code the compiler type-checks, so the error is the compiler's, at the
author's span:

| Declared | Emitted for the compiler |
|---|---|
| `returns = { available: AccountState.available }` | `const _: fn(&AccountState) -> &Money = \|v\| &v.available;` |
| `#[requires(view(AccountState))]` | `const _: () = assert_with_instance::<AccountState>();` (a trait bound on the view's consistency) |
| `compose = [core::AccountState]` | `const _: fn(core::AccountState) = \|_\| ();` |
| `linked(via = OffsetLink, ..)` | `const _: () = assert_relation::<OffsetLink, Account>();` |

What cannot be expressed as types (that a CEL invariant reads only declared
fields, that bounds are compatible with `model.lock`) is checked by
`cargo tkone check` and by the language server, from the schemas the build
exports.

## 4. Options are ordinary Rust; attributes mark items

An attribute with a grammar of its own (`levels(instance(within(..),
approve(when(..), by(..))))`) is a small language the editor does not know.
The rule: **an attribute marks an item; anything with options is a Rust
expression.** Parameters are the main case:

```rust
// before
#[parameter(kind = "commercial",
            levels(definition(default = "0.0075"),
                   instance(within = "default ± 0.005",
                            approve(when = "abs(new - old) > 0.0025", by = pricing_senior))))]
#[range(min = "0", max = "0.05")]
#[invariant("BASE.current + value <= CAP", reject = "above-cap")]
const MARGIN: Rate;

// after
#[parameter]
const MARGIN: Parameter<Rate> = Parameter::commercial()
    .definition(rate!(0.0075))
    .instance(Instance::within(rate!(0.005))
        .approve_when(cel!("abs(new - old) > 0.0025"), pricing_senior))
    .range(rate!(0), rate!(0.05))
    .invariant(cel!("BASE.current + value <= CAP"), "above-cap");
```

- **Every step completes.** `Parameter::` offers `commercial` and
  `operational`; `.instance(Instance::` offers `required`, `optional`,
  `within`. Wrong combinations are type errors: the builder's states are
  types, so `.within` cannot follow `required` if that makes no sense.
- **Values are typed literals.** `rate!(0.0075)`, `gbp!(1000.00)`,
  `Rounding::HalfUp`, `days(14)`: checked when written, not parsed at
  registration.
- **Complex defaults are struct literals,** not JSON in a string:

```rust
#[parameter]
const ERC: Parameter<ErcSchedule> = Parameter::commercial()
    .definition(ErcSchedule { bands: bounded![
        ErcBand { until_year: 1, rate: rate!(0.02) },
        ErcBand { until_year: 2, rate: rate!(0.01) },
    ] });
```

  The type's `#[range]` and `#[invariant]`s are checked against the literal
  when the crate builds: a band out of order fails the build, not
  registration.
- **The levels table reads off the builder:**

| Builder | Means |
|---|---|
| `.definition(v)` | set by the definition, default `v` |
| `.implementation(v)` | set by each implementation, default `v` |
| `.implementation_required()` | every implementation must set it |
| `.instance(Instance::optional())` | an account may override it |
| `.instance(Instance::required())` | every account must be given one when it opens |
| `.instance(Instance::within(d))` | an account may vary it by at most `d` from the default |
| `.approve(role)` | every change at that level needs that role's approval (C32) |
| `.approve_when(cel!(..), role)` | a change matching the condition needs that role's approval |

Field constraints on types stay attributes, because they mark a field:
`#[range(min = rate!(0), max = rate!(0.10))]`, `#[bounded(max = 10)]`.

## 5. Embedded languages are marked, and served

Three small languages are worth keeping: CEL (invariants, guards, `when`
filters, approval conditions), schedule rules, and `derive` formulas. They
are shorter and clearer than Rust for what they say. They are kept
**marked** and **checked**:

| Language | Written as | At compile time (macro) | By `cargo tkone check` and the language server |
|---|---|---|---|
| CEL | `cel!("..")`; or a `when`, `#[invariant]`, `#[guard]` argument | parsed: a syntax error is a compile error at the right column | type-checked against the model's schema: unknown fields, wrong types |
| Schedule rules | `schedules!`, `rule!` | parsed with tkone-schedule's grammar | hover: the next dates the rule produces |
| `derive` formulas | `derive = { remaining: ".." }` | parsed | type-checked against the composed views |
| Open references | `reference!("GBLO")`, `reference!("loans-uk")` | shape only | completed and checked against the registry: calendars, rate cards, accounting mappings |

## 6. Rust's own tools first

Where Rust already has the thing, use it, and the editor already supports
it:

- **`#[deprecated]` is Rust's.** A deprecated field is marked
  `#[deprecated(since = "3.2.0", note = "use `notice`")]` with
  `#[replaced_by(notice)]` for the tooling. Every editor strikes it through
  and every use warns, with no extension installed.
- **Doc comments are the catalogue.** `///` on a field becomes its schema
  description, its rustdoc, its hover, and the label in generated
  back-office forms.
- **Tests are `cargo test`.** Simulations, hook unit tests (with the
  generated `Ctx` builders) and snapshot tests of outcomes run with the
  standard runner and show in every editor's test explorer.

## 7. Macros tolerate half-written code

Completion is needed most while code is incomplete. A macro that fails on a
half-typed attribute stops expanding, and completion inside the item stops
with it. So every macro:

- **expands as far as it can** and adds a `compile_error!` at the exact span
  for what it could not parse, rather than failing whole;
- **says what was expected:**

```text
error: unknown hook kind `deliverd`
  --> products/cash-isa/src/lib.rs:14:12
   |
14 |     #[hook(deliverd(ledger::Entry))] fn credited;
   |            ^^^^^^^^ expected one of: scheduled, delivered(T), activation, closure, upgrade, correction
```

- **is tested for it:** each macro has tests on truncated input (every
  prefix of a valid example must expand without a panic), so tolerance does
  not regress.

---

## The language server

rust-analyzer serves everything that is Rust. A small language server,
`tkone-lsp`, serves what it cannot reach, side by side with it in any
editor that speaks the protocol (with a VS Code extension to install both):

| Feature | Where | From |
|---|---|---|
| Completion and type errors in CEL | `cel!`, `when`, `#[invariant]`, `#[guard]`, `approve` | the schemas the build exports (`target/tkone/*.json`) |
| Next dates on hover | rule strings in `schedules!` and `rule!` | tkone-schedule, with the definition's calendar |
| Completion of open references | `reference!(..)` | the registry, or a cached snapshot offline |
| Parameter hover | `#[parameter]` | levels, range, kind; current default per legal entity, from the registry |
| Compatibility warnings | a changed model field | `model.lock`: "this retypes field 7; it will be refused" |
| Quick fixes | a missing `#[requires]` input; an unmapped approval role | the same checks as registration |

The same diagnostics come from `cargo tkone check --message-format=json`,
so CI and agents that do not run an editor see exactly what a person sees.

## Where the cost goes

| Runs in | Does | Budget |
|---|---|---|
| Procedural macros (every expansion, in the editor) | parse; re-emit spans; generate types (`Action`, `Ctx`, `Rule`, `Schedules`); compiler checks for references; syntax of embedded languages | worth spending on: this is what the editor understands. Kept fast by not doing the rows below |
| `build.rs` and `cargo tkone` (on build) | schema export; `model.lock`; compatibility; WIT; the manifest | once per build, cached by Cargo |
| `tkone-lsp` (in the editor, beside rust-analyzer) | CEL types, rule dates, registry lookups | incremental, from exported schemas |
| CI | simulations over years; downstream checks for adopters | as long as they need |

- **Measured, not guessed.** Macro expansion time per item is tracked in
  the macros' benchmarks, and the generated code size per definition is
  reported by `cargo tkone check`. A regression is a failing check.
- **Generated code is small and plain.** Types and `const` checks, not
  logic: the compiler's work stays proportional to what was declared.
- **Generated once where possible.** A model crate generates its types once;
  products depend on the compiled crate, not on re-expanding the model.

## The workflow around the code

- **`cargo tkone new product --model term-deposits --implements term_deposit`**
  scaffolds a crate that compiles, with a passing simulation test.
- **`cargo tkone watch`** re-runs the product's simulations on save and
  shows changed bookings and events as a diff.
- **Snapshot tests** record a hook's `Outcome` and review changes like any
  other snapshot.
- **`cargo tkone context <definition>`** prints, in a compact form, what an
  author may use there: the model's types and fields, the permitted actions,
  the inputs `#[requires]` may name, the parameters. It is for agents as
  much as for people starting on an unfamiliar product.
- **Rustdoc for every model and interface,** published with the crates.

---

## Decisions

### C42. References are paths, checked by the compiler

Anything in a declaration that refers to something else (a model, an
interface, a definition, an extension, a balance type, an action, a state, a
role, an enum variant) is a Rust path, which macros re-emit with the
author's span where the compiler must resolve it. Strings remain for names
and the embedded languages. Deeper references (a view's field, a view's
consistency) become generated `const` checks
([1](#1-references-are-paths-not-strings),
[3](#3-references-across-crates-are-checked-by-the-compiler)).

### C43. Cargo owns versions and dependencies

A crate's version is its artifact's version; dependency ranges, resolution
and locking are Cargo's; registries are Cargo registries. Attributes never
repeat a version, and `tkone` adds only what Cargo does not know (schema
digests) ([2](#2-cargo-owns-versions-and-dependencies)).

### C44. Attributes mark items; options are ordinary Rust

Parameters are typed `const` builders with typed literal values; complex
defaults are struct literals, checked against their type's constraints when
the crate builds. Field constraints stay attributes, because they mark a
field. Rust's own attributes, doc comments and test runner are used wherever
they exist ([4](#4-options-are-ordinary-rust-attributes-mark-items),
[6](#6-rusts-own-tools-first)).

### C45. Embedded languages are marked, parsed at compile time, and served by a language server

CEL, schedule rules, `derive` formulas and open references are kept where
they are clearer than Rust, always in a marked position, parsed by the
macro at compile time, and type-checked by `cargo tkone check` and
`tkone-lsp` against the exported schemas and the registry
([5](#5-embedded-languages-are-marked-and-served),
[the language server](#the-language-server)).

### C46. Macros are tolerant, and editor capability is not traded for build time without measurement

Every macro expands as far as it can on incomplete input and reports what it
expected, tested on truncated input. Macro work is limited to what the
editor uses; the rest runs on build or in CI. Expansion time and generated
size are measured, and a capability is given up only against a measured
cost ([7](#7-macros-tolerate-half-written-code),
[where the cost goes](#where-the-cost-goes)).

---

## Open questions

- **Builder states as types.** Making invalid parameter combinations type
  errors needs a typestate builder; it reads well in completion, less well
  in error messages. Which combinations to forbid by type and which by a
  `const` assertion is decided when the SDK is built.
- **CEL types in rust-analyzer itself.** If CEL expressions were written as
  Rust closures over generated types, rust-analyzer would check them with
  no language server; but they must also run in the engine, outside Rust.
  A Rust subset compiled to CEL is possible, and not obviously worth it.
- **Editors beyond VS Code.** The language server works in any editor that
  speaks the protocol; packaging for each is a later concern.
