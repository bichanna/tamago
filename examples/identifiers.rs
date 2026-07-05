//! Identifier hygiene: coerce arbitrary strings into valid C identifiers, test
//! against C keywords, and mint fresh unique names.
//!
//! Run with: `cargo run --example identifiers`

use tamago::*;

fn main() {
    println!("// sanitize_ident:");
    for raw in ["hello world", "123abc", "struct", "ns::Vec", "my-var!", ""] {
        println!("  {:<14} -> {}", format!("{raw:?}"), sanitize_ident(raw));
    }

    println!("\n// is_c_keyword:");
    for name in ["return", "result", "_Bool", "bool", "counter"] {
        println!("  {name:<10} -> {}", is_c_keyword(name));
    }

    println!("\n// Gensym (fresh, unique names):");
    let mut g = Gensym::new();
    println!("  {}", g.fresh());
    println!("  {}", g.fresh());
    println!("  {}", g.fresh_named("index"));

    let mut labels = Gensym::with_prefix("L");
    println!("  {}", labels.fresh());
    println!("  {}", labels.fresh());
}
