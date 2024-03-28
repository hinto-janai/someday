#![doc = include_str!("../README.md")]
//---------------------------------------------------------------------------------------------------- Docs
#![cfg_attr(docsrs, feature(doc_cfg))]
//---------------------------------------------------------------------------------------------------- Lints
#![forbid(
    future_incompatible,
    let_underscore,
    break_with_label_and_loop,
    coherence_leak_check,
    deprecated,
    duplicate_macro_attributes,
    exported_private_dependencies,
    for_loops_over_fallibles,
    large_assignments,
    overlapping_range_endpoints,
    semicolon_in_expressions_from_macros,
    redundant_semicolons,
    unconditional_recursion,
    unused_allocation,
    unused_braces,
    unused_doc_comments,
    unused_labels,
    unused_unsafe,
    while_true,
    keyword_idents,
    missing_docs,
    non_ascii_idents,
    noop_method_call,
    unreachable_pub,
    single_use_lifetimes,
    variant_size_differences,
    unused_mut,
    unsafe_code
)]
#![deny(
    clippy::all,
    clippy::correctness,
    clippy::suspicious,
    clippy::style,
    clippy::complexity,
    clippy::perf,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::cargo,
    unused_comparisons,
    nonstandard_style
)]
#![allow(
    clippy::single_char_lifetime_names,
    clippy::implicit_return,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core,
    clippy::unwrap_used,
    clippy::min_ident_chars,
    clippy::absolute_paths,
    clippy::missing_inline_in_public_items,
    clippy::arithmetic_side_effects,
    clippy::unwrap_in_result,
    clippy::pattern_type_mismatch,
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    clippy::missing_trait_methods,
    clippy::pub_use,
    clippy::pub_with_shorthand,
    clippy::blanket_clippy_restriction_lints,
    clippy::exhaustive_structs,
    clippy::exhaustive_enums,
    clippy::panic,
    clippy::impl_trait_in_params,
    clippy::expect_used,
    clippy::redundant_pub_crate,
    clippy::type_complexity,
    clippy::module_name_repetitions,
    clippy::mod_module_files,
    clippy::module_inception,
    clippy::let_underscore_must_use,
    clippy::let_underscore_untyped,
    clippy::items_after_statements,
    clippy::single_call_fn,
    clippy::if_then_some_else_none
)]

//---------------------------------------------------------------------------------------------------- Mod
mod commit;
pub use commit::{Commit, CommitRef};

pub mod info;
pub use info::*;

mod reader;
pub use reader::Reader;

mod writer;
pub use writer::Writer;

mod transaction;
pub use transaction::Transaction;

mod patch;
pub use patch::Patch;

mod timestamp;
pub use timestamp::Timestamp;

mod free;
pub use free::{default, from_commit, new};
