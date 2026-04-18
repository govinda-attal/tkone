//! Declarative scheduling macros built on top of [`tkone_trigger`].
//!
//! This crate provides two attribute macros that eliminate the boilerplate
//! of wiring up [`tkone_trigger::Scheduler`] by hand.
//!
//! | Macro | Applied to | Purpose |
//! |-------|-----------|---------|
//! | [`#[schedule]`](macro@schedule) | `impl` block | Turn a plain struct into a scheduler entry point |
//! | [`#[job]`](macro@job) | `async fn` | Register a function as a job on a named scheduler |
//!
//! ## Quick start
//!
//! ### 1 — Define a scheduler struct
//!
//! Apply `#[schedule]` to an `impl` block. The block must contain exactly one
//! method marked `#[on_error]`; its parameter type becomes the shared error
//! type `E` for all jobs attached to this scheduler.
//!
//! ```rust,ignore
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
//!     async fn on_error(e: AppError) {
//!         eprintln!("job failed: {e}");
//!     }
//! }
//! ```
//!
//! `#[schedule]` generates three associated functions on `MySchedule`:
//!
//! ```text
//! fn  shutdown_token() -> CancellationToken
//! async fn run()
//! async fn run_until_signal()   // stops on Ctrl-C / SIGTERM
//! ```
//!
//! #### Attribute arguments
//!
//! | Argument | Required | Description |
//! |----------|----------|-------------|
//! | `spec = "..."` | yes | [`tkone_schedule`] time spec, e.g. `"1H:00:00"` |
//! | `fire_on_start` | no | Fire all jobs once immediately before the first tick |
//!
//! ### 2 — Register jobs
//!
//! Apply `#[job(SchedulerStruct)]` to any `async fn` that returns
//! `Result<(), E>`. The error type must match the one inferred from
//! `#[on_error]`.
//!
//! ```rust,ignore
//! use tkone_trigger_macros::job;
//!
//! # struct MySchedule;
//! # #[derive(Debug)] enum AppError { Msg(String) }
//! #[job(MySchedule)]
//! async fn do_work() -> Result<(), AppError> {
//!     println!("tick");
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
//! *Run `cargo run -p example-app --bin declarative` for the full program.*

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, parse::Parse, parse::ParseStream,
    Attribute, ImplItem, ImplItemFn, ItemImpl, ItemFn,
    LitStr, Meta, Path, Token, Type, FnArg,
    ReturnType, punctuated::Punctuated,
};

// ── #[schedule(spec = "...", tz = "...", fire_on_start)] ─────────────────────

struct ScheduleArgs {
    spec: LitStr,
    fire_on_start: bool,
}

impl Parse for ScheduleArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut spec: Option<LitStr> = None;
        let mut fire_on_start = false;

        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in &metas {
            match meta {
                Meta::NameValue(nv) if nv.path.is_ident("spec") => {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                        spec = Some(s.clone());
                    }
                }
                Meta::Path(p) if p.is_ident("fire_on_start") => {
                    fire_on_start = true;
                }
                Meta::NameValue(nv) if nv.path.is_ident("fire_on_start") => {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) = &nv.value {
                        fire_on_start = b.value;
                    }
                }
                _ => {}
            }
        }

        let spec = spec.ok_or_else(|| input.error("expected `spec = \"...\"`"))?;
        Ok(ScheduleArgs { spec, fire_on_start })
    }
}

/// Apply to an `impl` block to turn a plain struct into a scheduler entry point.
///
/// The impl block must contain exactly one method annotated `#[on_error]`.
/// That method must be `async fn on_error(e: ErrorType)` (no `self`).
///
/// # Generated items
///
/// On the struct:
/// - `fn shutdown_token() -> CancellationToken`
/// - `async fn run()`
/// - `async fn run_until_signal()`
///
/// # Example
///
/// ```rust,ignore
/// use tkone_trigger_macros::schedule;
///
/// struct Payments;
///
/// #[schedule(spec = "1H:00:00")]
/// impl Payments {
///     #[on_error]
///     async fn on_error(e: MyError) {
///         eprintln!("error: {e}");
///     }
/// }
/// ```
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

    // Find the #[on_error] method
    let on_error_fn = find_on_error_fn(&item_impl)?;
    let error_type = extract_on_error_param_type(on_error_fn)?;
    let on_error_ident = &on_error_fn.sig.ident;

    // Keep the original impl block but strip the #[on_error] helper attribute
    // so Rust doesn't complain about unknown attributes.
    let stripped_impl = strip_helper_attrs(&item_impl);

    let fire_on_start_call = if fire_on_start {
        quote! { scheduler = scheduler.fire_on_start(); }
    } else {
        quote! {}
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

            /// Build and run the scheduler until the iterator is exhausted or
            /// [`shutdown_token()`](Self::shutdown_token) is cancelled.
            pub async fn run() {
                let iter = ::tkone_schedule::time::SpecIteratorBuilder::new(
                    #spec_lit,
                    ::chrono::Utc,
                )
                .build()
                .expect("invalid schedule spec");

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
            fn handle_error(e: #error_type) -> ::std::pin::Pin<
                Box<dyn ::std::future::Future<Output = ()> + Send + 'static>
            > {
                Box::pin(Self::#on_error_ident(e))
            }
        }
    };

    Ok(expanded)
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

fn extract_on_error_param_type(f: &ImplItemFn) -> syn::Result<&Type> {
    // Expect exactly one parameter (no self): `e: ErrorType`
    let inputs: Vec<&FnArg> = f.sig.inputs.iter().collect();
    match inputs.as_slice() {
        [FnArg::Typed(pt)] => Ok(&pt.ty),
        _ => Err(syn::Error::new_spanned(
            &f.sig,
            "#[on_error] method must have exactly one parameter: `e: ErrorType`",
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
/// The function must return `Result<(), E>` where `E` matches the error type
/// of the named scheduler struct.
///
/// # Example
///
/// ```rust,ignore
/// use tkone_trigger_macros::job;
///
/// #[job(Payments)]
/// async fn process_payments() -> Result<(), MyError> {
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

    // Validate return type is Result<(), E>
    let error_type = extract_job_error_type(&item_fn)?;

    // Generate a unique helper fn name that won't clash with user code.
    let helper_ident = quote::format_ident!("__tkone_trigger_job_{}", fn_ident);

    let expanded = quote! {
        #item_fn

        // Plain fn so its pointer is a const expression (required by inventory::submit!).
        fn #helper_ident() -> ::tkone_trigger::BoxedFuture {
            ::std::boxed::Box::pin(async move {
                if let Err(e) = #fn_ident().await {
                    <#struct_path as ::tkone_trigger::ScheduleErrorHandler<#error_type>>::handle_error(e).await;
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
