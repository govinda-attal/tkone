//! Declarative scheduling macros built on top of
//! [`tkone_trigger`](https://docs.rs/tkone-trigger).
//!
//! This crate provides two attribute macros that eliminate the boilerplate
//! of wiring up `tkone_trigger::Scheduler` by hand.
//!
//! | Macro | Applied to | Purpose |
//! |-------|-----------|---------|
//! | [`#[schedule]`](macro@schedule) | `impl` block | Turn a plain struct into a scheduler entry point |
//! | [`#[job]`](macro@job) | `async fn` | Register a function as a job on a named scheduler |
//!
//! Every callback and error handler receives a `tkone_trigger::FireContext`
//! carrying the [`tkone_schedule::Occurrence`] for that tick, so handlers can
//! inspect the scheduled occurrence.
//!
//! ## Quick start
//!
//! ### 1 — Define a scheduler struct
//!
//! Apply `#[schedule]` to an `impl` block. The block must contain exactly one
//! method marked `#[on_error]`. That method takes `(ctx: FireContext, e: ErrorType)`;
//! the error type becomes the shared `E` for all jobs on this scheduler.
//!
//! ```rust,ignore
//! use tkone_trigger::FireContext;
//! use tkone_trigger_macros::schedule;
//! use thiserror::Error;
//!
//! #[derive(Debug, Error)]
//! enum AppError {
//!     #[error("{0}")]
//!     Msg(String),
//! }
//!
//! struct MySchedule;
//!
//! #[schedule(spec = "1H:00:00")]
//! impl MySchedule {
//!     #[on_error]
//!     async fn on_error(ctx: FireContext, e: AppError) {
//!         eprintln!("job failed at {:?}: {e}", ctx.occurrence().observed());
//!     }
//! }
//! ```
//!
//! `#[schedule]` generates three associated functions on `MySchedule`:
//!
//! ```text
//! fn  shutdown_token() -> CancellationToken
//! fn  describe() -> &'static str   // the spec in plain English, e.g. for a startup log line
//! async fn run()
//! async fn run_until_signal()   // stops on Ctrl-C / SIGTERM
//! ```
//!
//! #### Attribute arguments
//!
//! | Argument | Required | Description |
//! |----------|----------|-------------|
//! | `spec = "..."` | yes | [`tkone_schedule`] time spec, e.g. `"1H:00:00"` or `"HH:30M:00 ! 12:MM:SS"` |
//! | `start_spec = "..."` | no | From when on to fire: an RFC 3339 datetime (`"2026-06-01T09:00:00Z"`) **or** a time spec, meaning its next occurrence (`"09:00:00"` = the next 9 am). When omitted, from now. |
//! | `tz = "..."` | no | IANA time zone the spec is read in, e.g. `"Europe/London"`. When omitted, the zone written in the spec (`@2026-06-01T09:00[Europe/London]`), else UTC. |
//! | `fire_on_start` | no | Fire all jobs once immediately before the first tick |
//!
//! The spec, `start_spec` and `tz` are checked **while compiling**: a typo or
//! a date spec (`"YY-MM-DD"`, which this in-memory trigger doesn't support) is a
//! compile error on the attribute, not a panic at startup.
//!
//! A start written in the spec (`@…`) anchors its grid, for example the phase
//! of `HH:15M:00`; `start_spec` only decides from when on times are fired.
//! Past occurrences are never fired.
//!
//! ### 2 — Register jobs
//!
//! Apply `#[job(SchedulerStruct)]` to any `async fn` that takes
//! `ctx: FireContext` and returns `Result<(), E>`.
//!
//! ```rust,ignore
//! use tkone_trigger::FireContext;
//! use tkone_trigger_macros::job;
//!
//! # struct MySchedule;
//! # #[derive(Debug)] enum AppError { Msg(String) }
//! #[job(MySchedule)]
//! async fn do_work(ctx: FireContext) -> Result<(), AppError> {
//!     println!("tick at {:?}", ctx.occurrence().observed());
//!     Ok(())
//! }
//! ```
//!
//! Jobs are registered at link time via [`inventory`](https://docs.rs/inventory);
//! no explicit `add` calls are needed.
//!
//! ### 3 — Run
//!
//! ```rust,ignore
//! # async fn example() {
//! // Run until iterator exhausted or shutdown_token cancelled:
//! MySchedule::run().await;
//!
//! // Run until the above OR Ctrl-C / SIGTERM:
//! MySchedule::run_until_signal().await;
//! # }
//! ```
//!
//! ## Complete example
//!
//! *Run `cargo run -p tkone-trigger --example declarative_payments` for the full program.*

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, parse::Parse, parse::ParseStream,
    Attribute, ImplItem, ImplItemFn, ItemImpl, ItemFn,
    LitStr, Meta, Path, Token, Type, FnArg,
    ReturnType, punctuated::Punctuated,
};

// ── #[schedule(spec = "...", fire_on_start)] ─────────────────────────────────

struct ScheduleArgs {
    spec: LitStr,
    start_spec: Option<LitStr>,
    tz: Option<LitStr>,
    fire_on_start: bool,
}

impl Parse for ScheduleArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut spec: Option<LitStr> = None;
        let mut start_spec: Option<LitStr> = None;
        let mut tz: Option<LitStr> = None;
        let mut fire_on_start = false;
        let string = |nv: &syn::MetaNameValue| match &nv.value {
            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => Ok(s.clone()),
            other => Err(syn::Error::new_spanned(other, "expected a string literal")),
        };

        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in &metas {
            match meta {
                Meta::NameValue(nv) if nv.path.is_ident("spec") => spec = Some(string(nv)?),
                Meta::NameValue(nv) if nv.path.is_ident("start_spec") => start_spec = Some(string(nv)?),
                Meta::NameValue(nv) if nv.path.is_ident("tz") => tz = Some(string(nv)?),
                Meta::Path(p) if p.is_ident("fire_on_start") => {
                    fire_on_start = true;
                }
                Meta::NameValue(nv) if nv.path.is_ident("fire_on_start") => {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) = &nv.value {
                        fire_on_start = b.value;
                    }
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "unknown argument; expected `spec`, `start_spec`, `tz` or `fire_on_start`",
                    ))
                }
            }
        }

        let spec = spec.ok_or_else(|| input.error("expected `spec = \"...\"`"))?;
        Ok(ScheduleArgs { spec, start_spec, tz, fire_on_start })
    }
}

/// Apply to an `impl` block to turn a plain struct into a scheduler entry point.
///
/// The impl block must contain exactly one method annotated `#[on_error]`.
/// That method must be `async fn on_error(ctx: FireContext, e: ErrorType)` (no `self`).
///
/// # Attribute arguments
///
/// | Argument | Required | Description |
/// |---|---|---|
/// | `spec = "..."` | yes | `tkone-schedule` time spec, e.g. `"1H:00:00"` |
/// | `start_spec = "..."` | no | From when on to fire: RFC 3339 datetime **or** a time spec (its next occurrence). Omit to start from now. |
/// | `tz = "..."` | no | IANA time zone, e.g. `"Europe/London"`. Omit to use the zone written in the spec, else UTC. |
/// | `fire_on_start` | no | Run all jobs once immediately before the first tick |
///
/// The spec, `start_spec` and `tz` are checked while compiling.
///
/// # Generated items
///
/// On the struct:
/// - `fn shutdown_token() -> CancellationToken`
/// - `fn describe() -> &'static str`: the spec in plain English
/// - `async fn run()`
/// - `async fn run_until_signal()`
///
/// # Examples
///
/// Default — start from now:
/// ```rust,ignore
/// #[schedule(spec = "1H:00:00", fire_on_start)]
/// impl Payments { /* … */ }
/// ```
///
/// Start from the next occurrence of 9 am (time-spec start):
/// ```rust,ignore
/// #[schedule(spec = "1H:00:00", start_spec = "09:00:00")]
/// impl Payments { /* … */ }
/// ```
///
/// Start from a fixed moment (RFC 3339 start):
/// ```rust,ignore
/// #[schedule(spec = "HH:30M:00", start_spec = "2026-06-01T09:00:00Z")]
/// impl Snapshots { /* … */ }
/// ```
///
/// London time, every 15 minutes from 08:00 to 17:59, except the 12:00 hour:
/// ```rust,ignore
/// #[schedule(spec = "08-17:15M:00 ! 12:MM:SS", tz = "Europe/London")]
/// impl Prices { /* … */ }
/// ```
///
/// Only time specs are supported: monthly or business-day schedules
/// (`"YY-1M-L~NBT11:00:00"`) are rejected while compiling, because an
/// in-memory trigger would lose a missed occurrence on restart. Use
/// `tkone-tempo` for those.
#[proc_macro_attribute]
pub fn schedule(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ScheduleArgs);
    let item_impl = parse_macro_input!(item as ItemImpl);

    match expand_schedule(args, item_impl) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_schedule(args: ScheduleArgs, item_impl: ItemImpl) -> syn::Result<TokenStream2> {
    let struct_ty = &item_impl.self_ty;
    let spec_lit = &args.spec;
    let fire_on_start = args.fire_on_start;
    let description = check_args(&args)?;

    let on_error_fn = find_on_error_fn(&item_impl)?;
    let error_type = extract_on_error_error_type(on_error_fn)?;
    let on_error_ident = &on_error_fn.sig.ident;

    let stripped_impl = strip_helper_attrs(&item_impl);

    let fire_on_start_call = if fire_on_start {
        quote! { scheduler = scheduler.fire_on_start(); }
    } else {
        quote! {}
    };

    let optional = |lit: &Option<LitStr>| match lit {
        Some(lit) => quote! { ::std::option::Option::Some(#lit) },
        None => quote! { ::std::option::Option::None },
    };
    let (start_arg, tz_arg) = (optional(&args.start_spec), optional(&args.tz));
    let iter_build = quote! {
        let iter = ::tkone_trigger::time_schedule(#spec_lit, #start_arg, #tz_arg)
            .expect("the schedule spec was checked when compiling");
    };

    let expanded = quote! {
        #stripped_impl

        impl #struct_ty {
            /// Returns the global [`::tkone_trigger::CancellationToken`] for this scheduler.
            /// Cancel it to stop `run()` or `run_until_signal()`.
            pub fn shutdown_token() -> ::tkone_trigger::CancellationToken {
                static TOKEN: ::std::sync::OnceLock<::tkone_trigger::CancellationToken> =
                    ::std::sync::OnceLock::new();
                TOKEN.get_or_init(|| ::tkone_trigger::CancellationToken::new()).clone()
            }

            /// The schedule in plain English, as `tkone_schedule::time::describe`
            /// gives it; for example, for a startup log line.
            pub fn describe() -> &'static str {
                #description
            }

            /// Build and run the scheduler until the iterator is exhausted or
            /// [`shutdown_token()`](Self::shutdown_token) is cancelled.
            pub async fn run() {
                #iter_build

                let mut scheduler = ::tkone_trigger::Scheduler::new(iter, Self::#on_error_ident)
                    .with_shutdown_token(Self::shutdown_token());

                #fire_on_start_call

                for entry in ::tkone_trigger::inventory::iter::<::tkone_trigger::JobEntry>() {
                    if entry.schedule_type_id == ::std::any::TypeId::of::<#struct_ty>() {
                        scheduler.add_job(entry.func);
                    }
                }

                scheduler.run().await;
            }

            /// Run until the iterator is exhausted, [`shutdown_token()`](Self::shutdown_token)
            /// is cancelled, or SIGTERM / Ctrl-C is received.
            pub async fn run_until_signal() {
                let shutdown = Self::shutdown_token();
                let shutdown2 = shutdown.clone();

                tokio::select! {
                    _ = Self::run() => {}
                    _ = tokio::signal::ctrl_c() => {
                        shutdown2.cancel();
                    }
                }
            }
        }

        impl ::tkone_trigger::ScheduleErrorHandler<#error_type> for #struct_ty {
            fn handle_error(
                ctx: ::tkone_trigger::FireContext,
                e: #error_type,
            ) -> ::tkone_trigger::BoxedFuture {
                ::std::boxed::Box::pin(Self::#on_error_ident(ctx, e))
            }
        }
    };

    Ok(expanded)
}

/// Check the spec, `start_spec` and `tz` while compiling, and describe the
/// spec. Errors point at the offending argument.
fn check_args(args: &ScheduleArgs) -> syn::Result<String> {
    let spec = args.spec.value();
    if let Err(e) = tkone_schedule::time::describe(&spec) {
        let is_date = tkone_schedule::date::describe(&spec).is_ok() || tkone_schedule::datetime::describe(&spec).is_ok();
        let message = if is_date {
            format!(
                "`{spec}` is a date schedule; #[schedule] only takes time specs such as \"1H:00:00\", \
                 because an in-memory trigger would lose a missed date on restart (use tkone-tempo)"
            )
        } else {
            format!("invalid time spec `{spec}`: {e}")
        };
        return Err(syn::Error::new(args.spec.span(), message));
    }
    // Building checks what describing doesn't, such as the exclusions.
    let now = chrono::Utc::now();
    if let Err(e) = tkone_schedule::time::SpecIteratorBuilder::new_after(&spec, now).build() {
        return Err(syn::Error::new(args.spec.span(), format!("invalid time spec `{spec}`: {e}")));
    }
    if let Some(start) = &args.start_spec {
        let text = start.value();
        let is_datetime = chrono::DateTime::parse_from_rfc3339(&text).is_ok();
        if !is_datetime && tkone_schedule::time::describe(&text).is_err() {
            return Err(syn::Error::new(
                start.span(),
                format!("`start_spec` `{text}` is neither an RFC 3339 datetime nor a time spec"),
            ));
        }
    }
    if let Some(tz) = &args.tz {
        if tz.value().parse::<chrono_tz::Tz>().is_err() {
            return Err(syn::Error::new(tz.span(), format!("`{}` is not an IANA time zone", tz.value())));
        }
    }
    Ok(tkone_schedule::time::describe(&spec).unwrap_or_default())
}

fn find_on_error_fn(item_impl: &ItemImpl) -> syn::Result<&ImplItemFn> {
    let mut found: Option<&ImplItemFn> = None;
    for item in &item_impl.items {
        if let ImplItem::Fn(f) = item {
            if has_attr(&f.attrs, "on_error") {
                if found.is_some() {
                    return Err(syn::Error::new_spanned(
                        f,
                        "only one #[on_error] method is allowed per #[schedule] impl block",
                    ));
                }
                found = Some(f);
            }
        }
    }
    found.ok_or_else(|| {
        syn::Error::new_spanned(
            &item_impl.self_ty,
            "#[schedule] impl block must contain exactly one #[on_error] method",
        )
    })
}

fn extract_on_error_error_type(f: &ImplItemFn) -> syn::Result<&Type> {
    // Expect exactly two parameters (no self): `ctx: FireContext, e: ErrorType`
    let inputs: Vec<&FnArg> = f.sig.inputs.iter().collect();
    match inputs.as_slice() {
        [FnArg::Typed(_ctx_param), FnArg::Typed(err_param)] => Ok(&err_param.ty),
        _ => Err(syn::Error::new_spanned(
            &f.sig,
            "#[on_error] must have exactly two parameters: `ctx: FireContext, e: ErrorType`",
        )),
    }
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().is_ident(name))
}

fn strip_helper_attrs(item_impl: &ItemImpl) -> ItemImpl {
    let mut out = item_impl.clone();
    for item in &mut out.items {
        if let ImplItem::Fn(f) = item {
            f.attrs.retain(|a| !a.path().is_ident("on_error"));
        }
    }
    out
}

// ── #[job(StructType)] ───────────────────────────────────────────────────────

/// Register an async function as a job for a scheduler defined with `#[schedule]`.
///
/// The function must accept `ctx: FireContext` as its first parameter and return
/// `Result<(), E>` where `E` matches the error type of the named scheduler struct.
///
/// # Example
///
/// ```rust,ignore
/// use tkone_trigger::FireContext;
/// use tkone_trigger_macros::job;
///
/// #[job(Payments)]
/// async fn process_payments(ctx: FireContext) -> Result<(), MyError> {
///     println!("scheduled at {:?}", ctx.occurrence().observed());
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn job(args: TokenStream, item: TokenStream) -> TokenStream {
    let struct_path = parse_macro_input!(args as Path);
    let item_fn = parse_macro_input!(item as ItemFn);

    match expand_job(struct_path, item_fn) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_job(struct_path: Path, item_fn: ItemFn) -> syn::Result<TokenStream2> {
    let fn_ident = &item_fn.sig.ident;

    let error_type = extract_job_error_type(&item_fn)?;

    // Plain fn so its pointer is a const expression (required by inventory::submit!).
    let helper_ident = quote::format_ident!("__tkone_trigger_job_{}", fn_ident);

    let expanded = quote! {
        #item_fn

        fn #helper_ident(ctx: ::tkone_trigger::FireContext) -> ::tkone_trigger::BoxedFuture {
            ::std::boxed::Box::pin(async move {
                match #fn_ident(ctx.clone()).await {
                    Ok(()) => {}
                    Err(e) => {
                        <#struct_path as ::tkone_trigger::ScheduleErrorHandler<#error_type>>
                            ::handle_error(ctx, e).await;
                    }
                }
            })
        }

        ::tkone_trigger::inventory::submit! {
            ::tkone_trigger::JobEntry {
                schedule_type_id: ::std::any::TypeId::of::<#struct_path>(),
                func: #helper_ident,
            }
        }
    };

    Ok(expanded)
}

fn extract_job_error_type(f: &ItemFn) -> syn::Result<Type> {
    // Return type must be `-> Result<(), E>`
    let ReturnType::Type(_, ty) = &f.sig.output else {
        return Err(syn::Error::new_spanned(
            &f.sig,
            "#[job] function must return `Result<(), E>`",
        ));
    };

    let Type::Path(tp) = ty.as_ref() else {
        return Err(syn::Error::new_spanned(ty, "#[job] return type must be `Result<(), E>`"));
    };

    let last = tp.path.segments.last().ok_or_else(|| {
        syn::Error::new_spanned(ty, "#[job] return type must be `Result<(), E>`")
    })?;

    if last.ident != "Result" {
        return Err(syn::Error::new_spanned(ty, "#[job] return type must be `Result<(), E>`"));
    }

    let syn::PathArguments::AngleBracketed(ab) = &last.arguments else {
        return Err(syn::Error::new_spanned(ty, "#[job] Result must have type arguments"));
    };

    let args: Vec<_> = ab.args.iter().collect();
    match args.as_slice() {
        [syn::GenericArgument::Type(_unit), syn::GenericArgument::Type(err_ty)] => {
            Ok(err_ty.clone())
        }
        _ => Err(syn::Error::new_spanned(ty, "#[job] return type must be `Result<(), E>`")),
    }
}
