{{#include warning.md}}

> [!TIP]
> Always depend on **just a few** accurately selected crates, and explicitly 
> tune their features to avoid bloat.




# Dependencies
As you can read from our
<a href="https://github.com/pangiole/oauth2-client/blob/main/Cargo.toml" target="_blank">Cargo.toml</a>
file, our OAuth2 Client application is written against just a few dependencies, 
chosen amongst the most popular [Cargo](https://doc.rust-lang.org/cargo/) 
(Rust's package manager) crates nowadays, namely:

- [`anyhow`](https://crates.io/crates/anyhow)  
  a trait object based error type for easy idiomatic error handling;


- [`dotenvy`](https://crates.io/crates/dotenvy)  
  a library that loads environment variables from a .env file;


- [`tokio`](https://crates.io/crates/tokio)  
  the most dominant asynchronous runtime for I/O bound applications; plus a 
  few other _"tokio family"_ crates:

  - [`axum`](https://crates.io/crates/axum)  
    an HTTP routing and request-handling library, to easily implement 
    our OAuth2 `/callback` endpoint;

  - [`tracing`](https://crates.io/crates/tracing)  
    for instrumenting our application and collecting structured, event-based 
    diagnostic information (such as log records)


- [`openidconnect`](https://crates.io/crates/openidconnect)  
  a library for implementing our OIDC client, itself built on the 
  [`oauth2`](https://crates.io/crates/oauth2) crate;

  
When it comes to dependencies, rather than explain what each of them actually 
does, this chapter is more concerned on teaching _"self-restraint"_, especially 
when surrounded by abundance. 


## Bloating
In the Rust ecosystem, **dependency bloating** refers to the tendency for
projects to accumulate a massive, often disproportionate number of third-party
libraries (crates) — and in turn, massive compiled binary sizes and painfully 
long build times.

Because Cargo and [crates.io](https://crates.io/) make it exceptionally easy
to pull in external packages, developers frequently add more and more crates,
even for minor tasks rather than writing a few lines of native code by
themselves.

The key symptoms of dependency bloat are:

* **Exploding Dependency Trees**  
  A project depending on just 5 direct crates can easily pull in 100+ transitive
  dependencies (dependencies of our dependencies). Try to run:
  ```sh
  cargo tree
  ```
  and get impressed by how many `(crate, version)` pairs Cargo resolves by 
  transitively walking dependencies, even though only few were explicitly 
  named in the project `Cargo.toml` file.

* **Ballooning Binary Sizes**  
  Rust compiles code statically by default, and features like 
  [monomorphization](https://doc.rust-lang.org/book/ch10-01-syntax.html?highlight=monomorphization#performance-of-code-using-generics) 
  (generating specialized machine code for every generic type we use) can 
  cause unused or bloated generic functions to inflate our final executable.

  A complementary remedy — conditional compilation driven by Cargo feature
  flags — is covered later in the **Conditional Compilation** section.

  [//]: # (Add a link to my article: "Static vs Dynamic Linking in Rust)

* **Slow Compile Times**  
  Compiling a deep dependency tree taxes the Rust compiler heavily, greatly 
  increasing initial build times.


* **Security & Maintenance Overhead**  
  Every external crate expands our _"circle of trust"_. More dependencies mean
  more surface area for vulnerabilities, breaking changes, and supply-chain 
  risks.



## Features
One of the most effective remedies to reduce dependency bloating is tuning 
the **feature flags** for those few crates we intend to depend on, and that 
we explicitly named in our `Cargo.toml` file.

In Rust, [features](https://doc.rust-lang.org/cargo/reference/features.html) are
a mechanism provided by Cargo that allow a crate to offer optional 
functionality. Instead of bundling every possible capability into a library by 
default, authors use feature flags to let consumers opt into specific modules, 
transitive dependencies, or integrations with other libraries.

Under the hood, a Cargo feature is consumed by the crate's own source code
through **conditional compilation**. The author wraps the optional code in a
`#[cfg(feature = "...")]` attribute, so the Rust compiler only compiles that
code when the corresponding feature is switched on. In other words, enabling a
feature in `Cargo.toml` flips a compile-time switch: Cargo translates every
active feature into a `--cfg feature="..."` flag passed down to `rustc`, and
`rustc` in turn keeps or discards each `#[cfg(...)]`-gated block accordingly.

By default, many popular Rust crates are configured with "fat" **default 
features** meaning they turn on every bell and whistle so they work 
out-of-the-box for the widest possible audience. If we don't tune them, we can 
accidentally pull in massive transitive dependency trees for features our 
application never touches. For example, this is how we disable the 
`default-features` of the `anyhow` crate that our application depends on:

```toml
[dependencies.anyhow]
    version = "1.0.104"
    default-features = false
```

Setting `default-features = false` tells Cargo to ignore the author's default 
bundle, and leave the **base implementation** as the only part visible to our 
application.

Now if we don't explicitly opt back into at least some features, the crate will
effectively compile down to the base implementation only, and our project may 
fail to compile — or lose functionality we rely on. Therefore, once we have 
disabled the default features, we may need to tune the **feature flags** for 
those parts we actually need. 

For example, our application depends on the `axum` crate which, at version 
`0.8.9`, by default, would pull in 9 features (namely `form`, `http1`, 
`json`, `matched-path`, `original-uri`, `query`, `tokio`, `tower-log`, and 
`tracing`). Actually, our application does not need that much. All it needs 
is just a few:

```toml
[dependencies.axum]
    version = "0.8.9"
    default-features = false
    features = [
        "query",
        "tokio",
        "http1"
    ]
```

Sometimes the default features defined by the authors' crate happen to be
exactly what we want. For example the `openidconnect` crate, as we can read
their `Cargo.toml` file published at
<https://crates.io/crates/openidconnect/4.0.1/code/Cargo.toml#L151-L154>
defines `reqwest` and `rustls-tls` as default features — exactly the two we
need. Our manifest still spells them out explicitly, so the intent survives
even if the crate's defaults change later:

```toml
[dependencies.openidconnect]
    version = "4.0.1"
    default-features = false
    features = [
        "reqwest",
        "rustls-tls"
    ]
```


Tuning feature flags helps us trim the fat through several mechanisms:

* **Disabling Unused Transitive Dependencies**  
  If a crate depends on a heavy database driver or an optional TLS library 
  behind a feature flag, the code that `use`s those downstream crates is itself
  wrapped in a `#[cfg(feature = "...")]` block. Disabling the feature flips that
  conditional-compilation switch off, so the `use` statements and every call
  into those libraries are never compiled. When such a library is an *optional*
  dependency (turned on only by that feature), Cargo skips building it
  altogether.

* **Faster Compilation Times**  
  Conditional compilation discards inactive code *before* type-checking, so
  `rustc` and LLVM simply never see it. Every crate and feature combination
  eliminated means fewer lines of Rust code reach the compiler backend, which
  directly reduces your incremental and clean build times.

* **Smaller Binary Footprint**  
  Code hidden behind an inactive `#[cfg(...)]` predicate is not compiled at all,
  so there is nothing to link into the final executable. Fewer active code paths
  and zero unused dependencies yield a leaner binary.


## Conditional Compilation
Cargo features and conditional compilation are two faces of the same coin: the
former is declared in our `Cargo.toml`, the latter is applied in the crate's
Rust source code. Behind the scenes, every active feature is handed down to the
compiler as a `--cfg feature="..."` flag, and the crate author pairs each
feature with one or more
[`#[cfg(...)]`](https://doc.rust-lang.org/reference/conditional-compilation.html)
attributes that wrap the optional code:

```rust
// Compile this module only when the "tls" feature is enabled.
#[cfg(feature = "tls")]
mod tls_transport {
    // ...
}
```

Because `#[cfg(...)]` is evaluated *before* type-checking, switching a feature
off makes `rustc` discard the gated code altogether: it is never type-checked,
never sent to LLVM, and never linked into the final binary. That is precisely
why tuning the feature flags above translates so directly into faster builds
and smaller executables.

For completeness, Rust also ships the
[`cfg!`](https://doc.rust-lang.org/reference/conditional-compilation.html) macro,
which expands to a `true`/`false` literal at compile time instead of removing
code:

```rust
eprintln!("TLS support compiled in: {}", cfg!(feature = "tls"));
```

Unlike `#[cfg(...)]`, `cfg!` does **not** remove any code: it simply expands to
`true` or `false`, so both branches are still type-checked and compiled. It is
therefore best reserved for diagnostics — such as logging which capabilities
were compiled in — rather than for gating heavy code paths.

