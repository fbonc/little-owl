# Rust crash course

Written for someone fluent in C++ and Python, starting from nothing installed. The framing throughout is comparative, because almost every Rust concept is a variation on something you already know from one of those two languages, and the fastest way in is to name which one.

Read the first two sections before you write any code. The rest is reference you can skim and return to.

## 1. What Rust actually is

Rust is an ahead-of-time compiled, statically typed systems language with no garbage collector and essentially no runtime. On the implementation axis it sits almost exactly where C++ sits: source goes through a compiler front end, then through LLVM, and out comes a native binary with no interpreter and no virtual machine. Performance characteristics are C++ characteristics. There is no equivalent of the CPython interpreter loop, no bytecode, no GIL, and no reference counting unless you explicitly ask for it.

What makes it a different language from C++ is not the code generation, which is broadly similar, but that the compiler statically proves memory safety and thread safety before it will emit anything. C++ gives you RAII and expects discipline. Rust encodes the discipline in the type system and rejects programs that violate it. That is the whole trade: you spend time up front satisfying a checker, and in exchange you do not get use-after-free, double-free, data races, iterator invalidation, or null dereferences. Not "rarely get." The categories are closed by construction in safe code.

### Where the three languages differ

| Dimension | Python | C++ | Rust |
|---|---|---|---|
| Execution | Interpreted bytecode on a VM | Compiled to native | Compiled to native, via LLVM |
| Memory management | GC plus reference counting | Manual, conventionally RAII | Ownership, checked at compile time, freed deterministically |
| Runtime | Large: interpreter, GC, GIL | Tiny | Tiny, comparable to C++ |
| Type checking | Runtime, optional annotations | Compile time | Compile time, with inference |
| Assignment default | Binds a reference | Copies | Moves |
| Generics | Duck typing at runtime | Templates, duck typed, errors at instantiation | Monomorphized, bounded by traits, errors at definition |
| Polymorphism | Inheritance plus duck typing | Inheritance plus virtual | Traits only, no inheritance |
| Errors | Exceptions | Exceptions | Values, `Result<T, E>` |
| Absent value | `None`, any reference nullable | `nullptr`, raw pointers nullable | `Option<T>`, no null at all |
| Null safety | Runtime `AttributeError` | Undefined behaviour | Does not typecheck |
| Data races | Prevented by the GIL, at a cost | Your problem | Compile error |
| Build and deps | pip, venv, PyPI | CMake plus vcpkg or Conan, headers | Cargo, one tool, crates.io |
| Stable ABI | N/A | Platform-dependent, mostly yes | No |

Two rows deserve immediate elaboration because they cause the most early confusion.

### Move is the default

In C++, `auto b = a;` copies `a`, and you opt into moving with `std::move`. In Rust the default is inverted. `let b = a;` moves out of `a`, and afterwards `a` is statically unusable. This is not a runtime flag; the compiler tracks it and refuses to compile a later use.

```rust
let a = String::from("hello");
let b = a;              // a is moved into b
// println!("{a}");     // compile error: value borrowed after move
```

Types that are trivially copyable opt into copying instead, by implementing the `Copy` trait. That covers integers, floats, `bool`, `char`, shared references, and tuples or arrays of those. So integers behave the way you expect and heap-owning types like `String` and `Vec<T>` move. To duplicate a heap-owning value you call `.clone()`, which is explicit and visible in review, which is the point.

Coming from Python this is the larger adjustment, because in Python assignment never copies and never invalidates. Coming from C++ you already have the machinery in your head and only need to flip the default.

### References are borrows, and borrows have rules

`&T` is a shared reference and behaves like `const T&`. `&mut T` is an exclusive reference and behaves like `T&`, with one extra guarantee that C++ does not give you: while a `&mut T` exists, no other reference to that value may exist at all.

The rule, which is the entire borrow checker in one line: at any point, a value may have any number of shared references or exactly one exclusive reference, never both.

```rust
let mut v = vec![1, 2, 3];
let first = &v[0];       // shared borrow of v
v.push(4);               // needs &mut v; compile error, first is still alive
println!("{first}");
```

That is iterator invalidation, caught at compile time. The equivalent C++ is undefined behaviour that usually works until it doesn't.

Borrows end at their last use, not at the end of the enclosing scope. This matters constantly in practice, and the machinery is called non-lexical lifetimes. Move the `println!` above the `push` and the program compiles, because `first` is dead by then.

### Lifetimes

Lifetimes are the compiler's names for how long borrows are valid. They exist only at compile time and generate no code. You will see them as annotations like `&'a str`, and their job is to let the compiler verify across function boundaries that a returned reference cannot outlive what it points into.

```rust
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

That says the returned reference is valid for as long as both inputs are. Most of the time you write no lifetime annotations at all, because elision rules infer them. Do not attempt to deeply understand lifetime syntax early. Write code that returns owned values, hit the cases where the compiler asks for an annotation, and learn them then.

### The pointer and container map

Your C++ vocabulary translates almost directly.

| C++ | Rust | Notes |
|---|---|---|
| `T` on the stack | `T` | Same. |
| `std::unique_ptr<T>` | `Box<T>` | Unique heap ownership, no overhead beyond the pointer. |
| `std::shared_ptr<T>` | `Rc<T>` | Reference counted, non-atomic, single-thread only. |
| `std::shared_ptr<T>` across threads | `Arc<T>` | Atomic refcount. You pick the cost explicitly. |
| `std::weak_ptr<T>` | `Weak<T>` | Breaks reference cycles. |
| `const T&` | `&T` | |
| `T&` | `&mut T` | Plus exclusivity. |
| `T*` | `*const T` / `*mut T` | Raw pointers, only dereferenceable in `unsafe`. |
| `std::vector<T>` | `Vec<T>` | |
| `std::array<T, N>` | `[T; N]` | Size is part of the type. |
| `std::span<T>` | `&[T]` | A slice: pointer plus length. |
| `std::string` | `String` | Owned, heap, guaranteed UTF-8. |
| `std::string_view` | `&str` | Borrowed UTF-8 slice. |
| `std::unordered_map` | `HashMap<K, V>` | |
| `std::map` | `BTreeMap<K, V>` | Sorted. |
| `std::optional<T>` | `Option<T>` | But there is no null to fall back on. |
| `std::variant<A, B>` | `enum` | Far more ergonomic, see below. |
| `std::mutex` | `Mutex<T>` | Wraps the data rather than sitting beside it. |

Note the `Mutex<T>` row. In C++ a mutex is a separate object and nothing stops you touching the data without locking. In Rust the data lives inside the mutex and the only way to reach it is to lock, which returns a guard. Forgetting to lock is not a bug you can write.

### Interior mutability

The borrow rules are sometimes too strict for a legitimate design, most often shared-mutable graph structures. The escape hatch is to move the borrow check from compile time to run time using `RefCell<T>`, usually as `Rc<RefCell<T>>`. `borrow()` and `borrow_mut()` then panic if you violate exclusivity at runtime. `Cell<T>` is the simpler version for `Copy` types.

Reach for this when the design genuinely needs shared mutation, not to escape a borrow error you have not understood. In practice, most borrow errors from a C++ background are the compiler correctly identifying that your ownership story is unclear.

### No exceptions

Rust has no exceptions and no `try`/`catch`. Recoverable failure is a return value, `Result<T, E>`, and the `?` operator makes propagating it as terse as an exception would be. There is a second mechanism, `panic!`, which unwinds the stack and aborts the thread, but it is for unrecoverable bugs like an out-of-bounds index, not for control flow. Treat a panic the way you would treat an assertion failure.

### Threads

`Send` and `Sync` are marker traits the compiler applies automatically, describing whether a type can move between threads and whether it can be shared by reference across threads. Because they participate in normal type checking, passing a non-thread-safe type into a thread is an ordinary compile error. This is why Rust does not need a GIL to be memory-safe under concurrency, and it is the single largest practical advantage over C++.

### What you pay

Compile times are meaningfully worse than C, comparable to heavy template C++, and vastly worse than Python's nothing. There is no stable ABI, so dynamic linking between Rust libraries built by different compiler versions is not a thing, and most code statically links. There is no REPL in the standard toolchain. And the learning curve is genuinely front-loaded: the borrow checker will reject correct-looking programs for a few weeks.

## 2. Toolchain and workflow

### Install

One command installs everything, on all three platforms.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Windows, download `rustup-init.exe` from `rustup.rs` instead, and install the Visual Studio C++ build tools when it asks, because Rust uses the platform linker.

Restart your shell, then verify:

```sh
rustc --version
cargo --version
```

This installed `rustup`, which manages toolchain versions, roughly what `pyenv` does for Python. You will rarely invoke `rustc` directly, the same way you rarely invoke `gcc` directly on a CMake project.

Add the editor language server and the two lint tools:

```sh
rustup component add rust-analyzer clippy rustfmt
```

Then install the rust-analyzer extension in your editor. This is not optional in the way it might be for Python. Rust's type inference means a good chunk of useful information, particularly inferred types and trait method availability, is only visible through the language server. Working without it is dramatically harder.

### Cargo is the whole build system

Cargo is the piece with no real single-tool equivalent in either language you know. It is build system, dependency manager, test runner, benchmark runner, documentation generator, and publishing tool in one binary. Against your existing tooling it covers what CMake, vcpkg, and CTest do together, or what pip, venv, setuptools, and pytest do together.

The commands you will actually use:

```sh
cargo new myapp            # create a project
cargo check                # typecheck without codegen; fast, use constantly
cargo build                # debug build, into target/debug/
cargo build --release      # optimized build, into target/release/
cargo run                  # build and run the debug binary
cargo run --release        # build and run optimized
cargo test                 # run all tests
cargo add tokio            # add a dependency and edit Cargo.toml for you
cargo fmt                  # canonical formatting, no configuration debates
cargo clippy               # lints; far more opinionated and useful than the compiler
cargo doc --open           # build and open docs for your crate and all deps
```

The workflow habit worth forming immediately: `cargo check` in a loop rather than `cargo build`. It runs the whole front end including borrow checking and skips code generation, so it is several times faster. You only need a real build to run something.

Debug builds are unoptimized and slow, sometimes by more than an order of magnitude. If you benchmark a debug build you will conclude Rust is slow. Always measure `--release`.

### What a project consists of

```sh
cargo new wordcount
cd wordcount
```

produces:

```
wordcount/
  Cargo.toml       # manifest: name, version, edition, dependencies
  Cargo.lock       # exact resolved versions, generated
  .gitignore
  src/
    main.rs        # entry point for a binary crate
```

`Cargo.toml` is the declaration and `Cargo.lock` is the pinned resolution, exactly the `pyproject.toml` and `poetry.lock` relationship. Commit the lock file for applications, and conventionally not for libraries.

```toml
[package]
name = "wordcount"
version = "0.1.0"
edition = "2024"

[dependencies]
```

Editions are a compatibility mechanism with no analogue in either language you know. They let Rust make small breaking syntax changes without splitting the ecosystem: each crate declares its edition, the compiler honours it per crate, and crates of different editions link together freely. `2024` is current. There is no Python 2 to 3 situation possible here by design.

Dependencies come from `crates.io`, which is PyPI's equivalent. `cargo add serde` writes the version into `Cargo.toml` for you, and there is no separate install step because `cargo build` fetches and compiles what the manifest declares. There is no virtual environment, because dependencies are per project in `target/` rather than installed globally.

### Crates, modules, and the absence of headers

Two words to get straight, because the vocabulary does not match C++.

A crate is a compilation unit, and it is much larger than a C++ translation unit. It is closer to a whole library or a whole executable: the compiler reads the entire crate at once. A binary crate has `src/main.rs` with a `fn main()`. A library crate has `src/lib.rs` and no `main`. A package can contain one library and any number of binaries.

A module is a namespace inside a crate, and modules are where the real difference lands. There are no headers, no `#include`, no forward declarations, no include guards, and no separate declaration and definition. A file is a module. Declaration order does not matter anywhere, so a function can call one defined below it. This removes an entire category of C++ busywork.

```rust
// src/main.rs
mod capture;              // pulls in src/capture.rs
mod vault;                // pulls in src/vault.rs, or src/vault/mod.rs

use capture::Capturer;    // bring a name into scope

fn main() {
    // ...
}
```

Everything is private by default, including to parent modules. `pub` exposes an item, and there are finer grades, `pub(crate)` being the common one for something visible throughout your crate but not to consumers.

For multi-crate projects, a workspace shares one `target/` directory and one lock file across several crates, which is how the layout in `../architecture.md` is meant to be built:

```toml
# Cargo.toml at the repository root
[workspace]
members = ["crates/*"]
resolver = "3"
```

### Tests live in the source file

Rust puts unit tests in the same file as the code, gated behind an attribute so they are excluded from normal builds. This feels wrong from a C++ background and is very pleasant in practice.

```rust
pub fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_lowercases() {
        assert_eq!(normalize("  Hello "), "hello");
    }
}
```

`cargo test` runs those, plus integration tests from a top-level `tests/` directory, plus doc tests, which are code examples inside documentation comments that are compiled and executed. Doc tests mean your documentation cannot silently rot, and they are a genuinely novel idea worth knowing about early.

## 3. Syntax

Enough to read and write real code. Everything here is standard library, no dependencies.

### Bindings

```rust
let x = 5;                  // immutable, type inferred as i32
let mut y = 10;             // mutable
let z: u64 = 100;           // explicit type
const MAX: u32 = 1_000;     // compile-time constant, type required

let x = "shadowed";         // legal: shadowing, a new binding reusing the name
```

Immutable by default is the inverse of C++. There is no `const` keyword sprinkled through your code, because `const` is the default and `mut` is the annotation.

### Primitives

`i8` through `i128`, `u8` through `u128`, plus `isize` and `usize` which are pointer-width. `usize` is what indexing and lengths use, so you will see it constantly. Floats are `f32` and `f64`. `bool` and `char`, where `char` is a 4-byte Unicode scalar value rather than a byte.

There are no implicit numeric conversions at all, not even widening. Convert with `as` for primitives, or `.into()` and `.try_into()` when you want the conversion to be checked.

```rust
let a: u8 = 250;
let b: u32 = a as u32;         // explicit
let c: u32 = a.into();         // infallible widening
let d: u8 = 300_u32.try_into().unwrap();   // fallible, returns Result, panics here
```

One gotcha with no C++ equivalent, worth internalizing now: integer overflow panics in debug builds and wraps in release builds. That asymmetry is deliberate, catching bugs in development without paying for checks in production, but it means an overflow bug can pass your tests and behave differently in the shipped binary. Use `checked_add`, `saturating_add`, or `wrapping_add` when overflow is a real possibility rather than a bug.

### Expressions

Nearly everything is an expression that produces a value, more so than in C++. A block evaluates to its final expression when that expression has no semicolon. There is no ternary operator because `if` already is one.

```rust
let n = 7;
let label = if n % 2 == 0 { "even" } else { "odd" };

let squared = {
    let t = n * n;
    t                    // no semicolon: this is the block's value
};
```

The trailing-semicolon rule is the single most common early syntax error. A semicolon discards the value and yields `()`, the unit type, which is Rust's `void`.

### Functions

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b                    // no return keyword needed
}

fn log(msg: &str) {          // no arrow means it returns ()
    println!("{msg}");
}
```

Parameter types are always required; there is no inference across function boundaries. There is no function overloading and no default arguments. Where C++ would overload, Rust uses a trait with a generic parameter. Where C++ would default an argument, Rust uses `Option<T>` or a builder.

### Control flow

```rust
for i in 0..5 { }              // 0,1,2,3,4
for i in 0..=5 { }             // inclusive
for item in &collection { }    // iterate by reference
for item in collection { }     // consumes it

while cond { }
loop { break; }                // infinite; loop can also break with a value

let x = loop { break 42; };    // x is 42
```

There is no three-clause C-style `for`. Iteration is over ranges and iterators, closer to Python's model than C++'s.

### Structs and impl

```rust
struct Span {
    text: String,
    start: usize,
}

struct Point(f64, f64);        // tuple struct, fields are .0 and .1
struct Marker;                 // unit struct, zero-sized

impl Span {
    // associated function, no self: this is the constructor convention
    fn new(text: String, start: usize) -> Self {
        Self { text, start }   // field shorthand when names match
    }

    fn len(&self) -> usize {           // borrows
        self.text.len()
    }

    fn push(&mut self, s: &str) {      // borrows mutably
        self.text.push_str(s);
    }

    fn into_text(self) -> String {     // consumes
        self.text
    }
}

let mut s = Span::new("hi".into(), 0);
s.push(" there");
println!("{}", s.len());
```

Data and methods are declared separately: `struct` for the layout, `impl` for the behaviour. You can have several `impl` blocks for one type. There is no `public`/`private` section, just `pub` per item.

The three receiver forms are the ownership story in miniature. `&self` reads, `&mut self` mutates, `self` consumes. Rust has no constructors as a language feature; `new` is a plain associated function and only a convention. There is no destructor keyword either, but implementing the `Drop` trait gives you exactly C++ destructor semantics, running at end of scope in reverse declaration order.

### Enums are the feature C++ lacks

This is where Rust will feel genuinely better than what you are used to. An `enum` is a tagged union, like `std::variant`, but with pattern matching and exhaustiveness checking that make it pleasant enough to use as a primary modelling tool rather than a last resort.

```rust
enum CaptureMethod {
    Accessibility,
    Clipboard { restored: bool },      // variants can carry named fields
    Ocr(u32),                          // or positional ones
}

fn describe(m: &CaptureMethod) -> String {
    match m {
        CaptureMethod::Accessibility => "ax".to_string(),
        CaptureMethod::Clipboard { restored: true } => "clipboard, restored".to_string(),
        CaptureMethod::Clipboard { restored: false } => "clipboard, dirty".to_string(),
        CaptureMethod::Ocr(ms) => format!("ocr in {ms}ms"),
    }
}
```

`match` must be exhaustive. Add a variant to the enum and every incomplete `match` becomes a compile error, which turns "find all the places that handle this" from a grep into a build. Use `_` as a catch-all when you genuinely want one.

The two most important enums are in the standard library.

```rust
enum Option<T> { Some(T), None }
enum Result<T, E> { Ok(T), Err(E) }
```

`Option<T>` replaces null entirely. A `&T` cannot be null, so absence has to be modelled in the type, which means the compiler forces you to handle it. Every `NullPointerException` and every segfault-from-null you have debugged is a compile error here.

### Pattern matching shorthands

Full `match` is often more than you need.

```rust
if let Some(v) = maybe_value {
    println!("{v}");
}

let Some(v) = maybe_value else {
    return;                     // let-else: bind or diverge
};
// v is in scope from here, unwrapped

while let Some(item) = stack.pop() { }

let (a, b) = (1, 2);            // destructuring
```

`let ... else` is the cleanest way to handle a guard clause and worth reaching for often.

### Error handling in practice

```rust
use std::fs;
use std::io;

fn read_config(path: &str) -> Result<String, io::Error> {
    let text = fs::read_to_string(path)?;    // ? returns early on Err
    Ok(text.trim().to_string())
}
```

`?` is the whole ergonomic story. On `Ok` it unwraps; on `Err` it returns early from the function, converting the error type via the `From` trait. It also works on `Option`, returning `None` early.

You will see `.unwrap()` and `.expect("message")` everywhere in examples. Both panic on failure. They are fine in tests, in prototypes, and where you can prove the failure is impossible. They are not fine on a path a user can reach, and `expect` with a message explaining why the failure is impossible is always better than a bare `unwrap`.

For applications, the ecosystem standard is two crates: `anyhow` for a single easy error type in application code, and `thiserror` for deriving structured error enums in libraries.

```sh
cargo add anyhow
```

```rust
use anyhow::{Context, Result};

fn load(path: &str) -> Result<String> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading config at {path}"))?;
    Ok(text)
}
```

`with_context` attaches a human-readable layer to an error as it propagates, giving you a causal chain rather than a bare `No such file or directory`.

### Traits

A trait is an interface. It is closest to a C++20 concept and an abstract base class fused into one thing, and it is Rust's only polymorphism mechanism, because there is no inheritance.

```rust
trait Summarize {
    fn summary(&self) -> String;

    fn preview(&self) -> String {          // default method, overridable
        let s = self.summary();
        match s.char_indices().nth(20) {
            Some((byte_idx, _)) => format!("{}...", &s[..byte_idx]),
            None => s,
        }
    }
}

struct Note { title: String }

impl Summarize for Note {
    fn summary(&self) -> String {
        self.title.clone()
    }
}
```

Generic code constrains type parameters with trait bounds. Unlike pre-concepts C++ templates, the bound is checked at the definition site, so errors point at your generic function rather than producing a page of instantiation backtrace.

```rust
fn print_all<T: Summarize>(items: &[T]) {
    for i in items {
        println!("{}", i.summary());
    }
}

// equivalent, with a where clause, better for several bounds
fn print_all2<T>(items: &[T]) where T: Summarize + Clone { }

// equivalent, argument-position impl Trait, terser
fn print_one(item: &impl Summarize) { }
```

Generics are monomorphized exactly like C++ templates: one specialized copy per concrete type, statically dispatched, fully inlinable. For runtime dispatch you ask for a trait object with `dyn`.

```rust
let items: Vec<Box<dyn Summarize>> = vec![Box::new(note)];
```

One implementation detail that differs from C++ and occasionally matters: `&dyn Trait` is a fat pointer, two words wide, carrying the data pointer and the vtable pointer separately. A C++ polymorphic object stores its vtable pointer inside the object. So Rust's non-polymorphic structs have no hidden vtable field and no size overhead, and you pay only where you opt in.

`derive` auto-implements common traits, which saves an enormous amount of boilerplate compared to writing comparison and printing operators by hand.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
struct Id(u64);
```

`Debug` gives you `{:?}` in format strings, which is your primary debugging tool. `Display` is the user-facing `{}` and is deliberately not derivable, because how a type should look to a user is a decision.

One rule that will eventually bite you: the orphan rule says you may implement a trait for a type only if you own the trait or the type. You cannot implement a third-party trait for a third-party type. The workaround is the newtype pattern, wrapping the foreign type in your own single-field struct.

### Closures and iterators

Closures look like Python lambdas but are real values with inferred captures.

```rust
let add = |a: i32, b: i32| a + b;
let factor = 3;
let scale = |x: i32| x * factor;      // captures factor by reference
let consume = move |x: i32| x * factor;  // move forces capture by value
```

`move` is what you need when a closure outlives its scope, most often when spawning a thread or an async task.

Iterators are lazy, compose like Python generators, and compile down like handwritten loops because monomorphization plus inlining removes the abstraction entirely. This is the canonical example of what "zero-cost abstraction" means.

```rust
let words = vec!["alpha", "be", "gamma", "de"];

let long: Vec<&str> = words
    .iter()
    .filter(|w| w.len() > 2)
    .copied()
    .collect();

let total: usize = words.iter().map(|w| w.len()).sum();
```

Nothing runs until a consuming operation such as `collect`, `sum`, or a `for` loop. `collect` is generic over the target container and is usually the place you need a type annotation, because the compiler cannot guess whether you wanted a `Vec`, a `HashSet`, or a `String`.

### String and &str

The one piece of the standard library that reliably confuses newcomers, so it is worth being explicit.

`String` is an owned, growable, heap-allocated, guaranteed-UTF-8 buffer. `&str` is a borrowed view into UTF-8 text. The relationship is exactly `std::string` to `std::string_view`, and the conversion `&String` to `&str` happens automatically through deref coercion, which is why a function taking `&str` accepts a `&String` without ceremony.

Take `&str` in function parameters and return `String` when you must produce new text. That default is right most of the time.

```rust
let owned: String = String::from("hello");
let owned2: String = "hello".to_string();
let owned3: String = "hello".into();
let borrowed: &str = &owned;
let slice: &str = &owned[0..3];       // byte indices, must land on char boundaries
```

Because the UTF-8 invariant is enforced, you cannot index a string by integer to get a character. `s[0]` does not compile. Iterate with `.chars()` for scalar values or `.bytes()` for bytes, and know that slicing by a byte range panics if the range splits a multi-byte character.

Filesystem paths are a third family, `Path` and `PathBuf`, standing in the same borrowed-to-owned relationship, and they are not strings because not every OS path is valid UTF-8.

### Unsafe

`unsafe` is narrower than its reputation. It does not disable the borrow checker or the type system. It unlocks exactly five operations: dereferencing a raw pointer, calling an `unsafe` function including any foreign function, accessing or modifying a mutable static, implementing an `unsafe` trait, and reading a union field.

You need it for FFI, which is why the platform accessibility work in `../architecture.md` will contain some. The convention is to keep `unsafe` blocks minimal and wrap them in a safe API whose invariants you document, which is precisely what crates like `windows` and `objc2` do for you.

### Macros

Anything ending in `!` is a macro. They are hygienic and operate on syntax trees rather than tokens, so they are much safer than the C preprocessor and can do things functions cannot, such as accept a variable number of typed arguments.

The ones you use daily:

```rust
println!("{} and {:?}", display_value, debug_value);
println!("{name} is {age}");                  // inline captures
let v = vec![1, 2, 3];
let s = format!("{x:.2}");                    // returns String
dbg!(&some_value);                            // prints file, line, value; returns it
assert_eq!(a, b);
todo!();                                      // typechecks as any type, panics if reached
```

`dbg!` and `todo!()` are both worth adopting immediately. `todo!()` in particular lets you sketch a whole module's signatures and let the compiler check the shape before writing any bodies.

## 4. A complete program

This is small but real, and it exercises most of the above: argument handling, error propagation, borrowing, the `HashMap` entry API, iterators, sorting, and formatting.

```sh
cargo new wordcount
cd wordcount
```

```rust
// src/main.rs
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;

fn main() -> Result<(), Box<dyn Error>> {
    // args() yields the program name first, so the file is at index 1.
    // nth returns Option; ok_or turns it into Result; ? propagates.
    let path = env::args().nth(1).ok_or("usage: wordcount <file>")?;

    let text = fs::read_to_string(&path)?;

    // Keys borrow from `text`, which outlives the map. No allocation per word.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }

    // into_iter consumes the map and yields owned (key, value) pairs.
    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();

    // Descending by count, then ascending by word for stable ties.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    for (word, n) in ranked.iter().take(10) {
        println!("{n:>6}  {word}");
    }

    Ok(())
}
```

```sh
cargo run -- src/main.rs
```

Four things in there are worth naming explicitly.

`main` returning `Result` is what lets you use `?` at the top level. On `Err` the runtime prints the `Debug` representation and exits nonzero.

`Box<dyn Error>` is a trait object holding any error type, which is why `?` accepts both the `&str` from `ok_or` and the `io::Error` from `read_to_string` in the same function. In real code you would use `anyhow::Result` instead, which does the same thing with better context support.

`counts.entry(word).or_insert(0)` returns `&mut usize`, so the leading `*` dereferences it to increment. The entry API is how you do get-or-insert in one hash lookup, and it is a pattern you will use constantly.

The `HashMap<&str, usize>` borrowing from `text` is the ownership system paying off. In Python or naive C++ you would allocate a string per word. Here the compiler proves the borrows are valid for the map's whole life, so you allocate nothing.

## 5. Async, briefly

Relevant because the project this sits next to uses it, and because it works differently from Python's asyncio in one important way.

An `async fn` returns a `Future`, which is a lazy state machine the compiler generates. `.await` drives it. Conceptually this is Python's coroutines and `await`, and the syntax is nearly identical.

The difference is that Rust ships no event loop. Python has `asyncio` in the standard library; Rust's standard library has the `Future` trait and nothing to run it. You choose a runtime, and in practice that means `tokio`.

```sh
cargo add tokio --features full
```

```rust
#[tokio::main]
async fn main() {
    let body = reqwest::get("https://example.com").await.unwrap()
        .text().await.unwrap();
    println!("{}", body.len());
}
```

`#[tokio::main]` is a macro that wraps your async `main` in a runtime startup. Async is a genuinely advanced corner of the language, particularly where it meets lifetimes and trait objects. Do not start here. Get comfortable with ownership and traits in synchronous code first.

## 6. Friction you should expect

Specific to your background, in rough order of how much time it will cost you.

Fighting the borrow checker over shared mutable state. Coming from C++ you will reach for a pattern that has two live paths to one mutable object, and it will be rejected. Nine times out of ten the fix is restructuring so ownership is clear, often by passing indices or IDs instead of references. The tenth time you genuinely need `Rc<RefCell<T>>`. Reaching for `Rc<RefCell<T>>` first is the classic beginner mistake and produces code that panics at runtime instead of failing to compile.

Self-referential structs, linked lists, and graphs are all disproportionately hard, because they are exactly what the ownership model is least suited to expressing. The idiomatic answer is usually an arena: put the nodes in a `Vec` and use indices as your edges. If you set out to learn Rust by writing a linked list you will have a bad time, and there is a well-known tutorial titled roughly to that effect.

`String` versus `&str`, and its cousins `PathBuf` versus `Path` and `OsString` versus `OsStr`. Costly for a week, then automatic.

The absence of function overloading and default arguments. You will miss both. The replacements are traits and builders, and they are more verbose.

Integer overflow differing between debug and release, described above. This is the one on this list most likely to cause an actual bug rather than just friction.

The orphan rule blocking an `impl` you want, solved with a newtype wrapper.

Compile times, particularly on a first build that compiles your whole dependency tree. `cargo check` while iterating is most of the mitigation.

Coming from Python specifically: the compiler will refuse programs you know are correct, and the answer is essentially never to fight it and essentially always to change the design. Rust's error messages are the best in any language I know of, frequently naming the exact fix. Read them completely rather than skimming for the line number, and run `cargo clippy` and read those too, because clippy teaches idiom rather than just correctness.

## 7. What to read next

The Book, at `doc.rust-lang.org/book`, is the official introduction and unusually good. It is the single best use of your next several hours. Chapters 4, 10, and 13, on ownership, traits and generics, and iterators, are the load-bearing ones.

Rustlings is a set of small failing programs you fix in order, installed with `cargo install rustlings`. It is the fastest way to convert reading into recall.

Rust by Example, at `doc.rust-lang.org/rust-by-example`, is the lookup-oriented companion to The Book.

The standard library documentation at `doc.rust-lang.org/std` is excellent and worth browsing rather than only searching. Every type documents its methods with runnable examples. `cargo doc --open` generates the same style of documentation for your own crate and its dependencies, which is how you should read third-party crates rather than reading their source.

A suggested first week: install everything, read Book chapters 1 through 6, work Rustlings through the ownership section, then write the word-count program above from scratch without copying it. That combination gets you to the point where the borrow checker is an occasional argument rather than a constant one, which is the threshold worth reaching before starting anything real.
