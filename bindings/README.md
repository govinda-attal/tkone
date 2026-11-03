# Language bindings

The Rust crates in [`crates/`](../crates/) are the implementation; each
directory here makes them available in another language.

```text
bindings/
  python/          the `tkone` Python package (PyO3 + maturin)
  <language>/      future: node/, java/, …
```

## Conventions

- **One directory and one package per language,** not per crate. A Python
  user installs `tkone` once.
- **One native library per language, with one module per crate.** Types
  then work across crates: a `tkone.calendar.Calendar` can be passed to
  `tkone.schedule.Schedule.occurrences(calendar=…)`. Separate native
  libraries could not share them.
- **Module names follow crate names:** `tkone-<name>` is `tkone.<name>` in
  Python (and `tkone/<name>` or `tkone.<name>` in other languages).

  | Crate | Python | Binding code |
  |-------|--------|--------------|
  | `tkone-schedule` | `tkone.schedule` | [`python/src/schedule.rs`](python/src/schedule.rs) |
  | `tkone-calendar` | `tkone.calendar` | [`python/src/calendar.rs`](python/src/calendar.rs) |

- **Outside the Cargo workspace.** Each binding builds with its language's
  tool (maturin for Python), so `cargo build` and `cargo test` across the
  repository never need Python or another runtime. The root `Cargo.toml`
  excludes `bindings/`.
- **Thin bindings.** Behaviour lives in the Rust crates and is tested
  there; a binding converts types and errors, and its tests check that the
  conversions are right.
- **Versioned with the crates.** A binding release names the crate versions
  it wraps.

## Adding a crate to the Python package

1. Add the crate to `python/Cargo.toml`.
2. Write `python/src/<name>.rs` with a `register(parent)` function that adds
   a `<name>` submodule, its classes with `#[pyclass(module = "tkone.<name>")]`,
   and its errors with `__module__` set to `tkone.<name>`.
3. Call it from `python/src/lib.rs`.
4. Add `python/python/tkone/<name>.py` re-exporting it, `<name>.pyi` with
   its types, and `<name>` to `tkone/__init__.py`.
5. Add `python/tests/test_<name>.py`.
