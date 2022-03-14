//! An X11 Window Manager that is basically a Rust clone of `bspwm`
#![allow(unused)]
#![deny(
    clippy::all,
    // clippy::cargo,
    clippy::complexity,
    clippy::correctness,
    clippy::nursery,
    clippy::pedantic,
    clippy::perf,
    clippy::restriction,
    clippy::style,
    absolute_paths_not_starting_with_crate,
    anonymous_parameters,
    bad_style,
    const_err,
    // dead_code,
    ellipsis_inclusive_range_patterns,
    exported_private_dependencies,
    ill_formed_attribute_input,
    improper_ctypes,
    keyword_idents,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_abi,
    // missing_debug_implementations,
    // missing_docs,
    no_mangle_generic_items,
    non_shorthand_field_patterns,
    noop_method_call,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    pointer_structural_match,
    private_in_public,
    pub_use_of_private_extern_crate,
    semicolon_in_expressions_from_macros,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unaligned_references,
    unconditional_recursion,
    unreachable_pub,
    unsafe_code,
    // unused,
    // unused_allocation,
    // unused_comparisons,
    // unused_extern_crates,
    // unused_import_braces,
    // unused_lifetimes,
    // unused_parens,
    // unused_qualifications,
    variant_size_differences,
    while_true
)]
#![allow(
    // Fill out documentation
    // clippy::missing_docs_in_private_items,

    // Find this problem
    clippy::pattern_type_mismatch,

    // ?
    clippy::redundant_pub_crate,

    clippy::as_conversions,
    clippy::blanket_clippy_restriction_lints,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cognitive_complexity,
    clippy::create_dir,
    clippy::doc_markdown,
    clippy::else_if_without_else,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::expect_used,
    clippy::exit,
    clippy::implicit_return,
    clippy::indexing_slicing,
    clippy::integer_arithmetic,
    clippy::integer_division,
    clippy::mod_module_files,
    clippy::multiple_inherent_impl,
    clippy::separated_literal_suffix,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated,
    clippy::similar_names,
    clippy::string_add,
    clippy::string_slice,
    clippy::struct_excessive_bools,
    clippy::too_many_lines,
    clippy::upper_case_acronyms,
    clippy::unreachable,
    clippy::unwrap_in_result,
    clippy::used_underscore_binding,
    clippy::single_char_lifetime_names,

    // Remove later
    clippy::print_stdout,
    clippy::use_debug,

    // clippy::single_match_else,
)]
#![cfg_attr(
    any(test),
    allow(
        clippy::expect_fun_call,
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_in_result,
        clippy::unwrap_used,
        clippy::wildcard_enum_match_arm,
    )
)]
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

mod cli;
mod config;
mod error;
mod events;
mod geometry;
mod input;
mod macros;
mod monitor;
mod stack;
mod types;
mod utils;
mod xcb_utils;
mod xconnection;

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashSet},
    process::exit,
    sync::Arc,
};

use anyhow::Result;
use config::Config;

use x11rb::{
    connection::Connection,
    errors::{ReplyError, ReplyOrIdError},
    protocol::{randr::ConnectionExt as _, xproto::ConnectionExt as _, ErrorKind, Event},
    wrapper::ConnectionExt as _,
    COPY_DEPTH_FROM_PARENT,
    CURRENT_TIME,
};

use xcb_utils::XUtility;
use xconnection::LWM;

fn main() -> Result<()> {
    // let (conn, screen_num) = XUtility::setup_connection()?;
    let config = Config::load_default()?;
    // let xconn = LWM::new(conn, screen_num, &config)?;

    println!("CONFIG: {:#?}", config);

    Ok(())
}
