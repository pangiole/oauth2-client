# Build dependencies in Rust
This article clarify the concept of build dependencies in Rust language and 
Cargo, by comparing it with other popular languages and build tools.

> **TL;DR**  
> - **Rust tools** never throws away a version another crate asked for. It 
>   merges semver-compatible dependencies into one version and, when 
>   those are semver-incompatible, it downloads and compiles **both** 
>   versions as independent crates — correctness over binary size.
> 
> 
> - **Java tools** always collapse to **one** JAR per artifact, chosen by
>   "nearest"/"highest"/"latest," regardless of compatibility — smaller
>   classpath, but silent, often runtime-exploding, mismatches.



## Dependencies
First, let's introduce the concept of build dependencies in general terms.

...

### Packaging
Let's introduce the concept of packaging dependencies.

...

### Versioning
Let's introduce the concept of versioning dependencies, particularly using 
the **SemVer** ([Semantic Versioning](https://semver.org/)) approach.

...

## Java tools 
This section summarizes how Java build tools, such as Maven, Gradle and SBT 
(Ivy), just to name a few, handle dependencies.

> **The Java CLASSPATH is flat**   
> All transitive JARs - Java ARchives land on the same compile/runtime 
> classpath, so your code technically *can* import a class from any transitive 
> jar. There's no compiler barrier.


### Conflict resolution
This is where Java tools and Rust tools genuinely diverge — they solve version
conflicts with **opposite philosophies**. Java's model is: _"pick exactly one 
version, silently discard the rest"_.

Maven, Gradle, and SBT all resolve a given `groupId:artifactId` to **a single 
version**. Two versions of the same jar never coexist on the classpath by 
default:

Tool	  | Default conflict rule
----------|-------------------------
Maven	  | "nearest definition" — the version closest to the root of the dependency tree wins; ties broken by declaration order
Gradle	  | "highest version wins"
SBT (Ivy) |	"latest revision wins" (configurable)

The loser is silently excluded. This is fine when the versions are compatible, 
but **semver is not enforced** — the tools don't know or care that `0.9` and 
`1.0` are breaking. So Maven will happily replace `serde 0.9` (that crate B 
actually needs) with `serde 1.0` (that crate A needs), and now B is running 
against an API it wasn't compiled against.

The consequence is the classic JVM pain you've probably heard of:

- **Compile-time:** `ClassNotFoundException`, `NoClassDefFoundError`, 
  or the wrong version winning.
- **Runtime:** `NoSuchMethodError`/`AbstractMethodError` when B calls a 
  method that existed in 0.9 but was removed/renamed in 1.0.

Because the classpath is **flat and ordered**, the JVM physically can't hold 
two `serde` jars and know which one each caller wants — class/package names 
would collide. (Workarounds exist: dependency `exclusions`, 
`<dependencyManagement>`, Gradle's `strictly`/`force`/`resolutionStrategy`, 
and jar *shading/relocation* to rename packages — but all of these are manual 
band-aids precisely because the default model can't express "two versions 
at once.")


## Rust tools
...

Cargo also has transitive dependencies, but unlike other technologies it 
enforces a hard rule that your code may only *name* crates you declared 
directly.

Each crate resolves its own set of dependencies, and the compiler enforces that
a crate can only use/reference crates listed in its own `Cargo.toml` file.
Transitive crates are _downloaded_ and _compiled_, but they are **invisible to 
your source code**

```toml
[package]
name = "my-project"

[dependencies]
axum = "0.8.9" # it internally depends on serde
```

So "_axum depends on serde_" means "_serde gets built_"; it does not mean 
"_serde is available to your crate_". If you try to compile the following 
piece of Rust

```rust
// file: myapp.rs
use  serde::Deserialize;

#[derive(Deserialize)]           // <- this macro expands *into your crate*
struct MyModel { ... }
```

without explicitely naming `serde` in your Cargo `[dependencies]` table, you 
get 

``` 
error [E0433]: failed to resolve: use of undeclared crate or module 'serde'`.
```

Cargo intentionally forces this explicitness so your crate's build doesn't 
silently break when a dependency stops pulling in a crate your were freeloading 
on. The `#[derive(Deserialize)]` macro expands to code inside `myapp.rs` 
that looks roughly like:

```rust
impl<'de> serde::Deserialize<'de> for MyModel {
    // ... serde::__private, serde::de::Error, etc.
}
```

That generated code lives in *your* crate, so it needs `serde` to be nameable 
*from your crate*. The macro can't reach into axum's private copy of serde. 
Hence, the direct dependency is required for the `derive` macro to compile — it 
has nothing to do with "_loading serde twice_".


### Conflict resolution
This is where Rust Cargo and the Java tools genuinely diverge — they solve 
version conflicts with **opposite philosophies**. Cargo's model is: _"unify 
what's compatible, duplicate what isn't"_.

Cargo's entire version resolution is built on _semver compatibility_, not on 
"latest wins" or "nearest wins". The rule is roughly: **one version per (crate, 
compatible range)**. Two requirements are "compatible" if they can both be 
satisfied by the same version under semver's `^` (caret) semantics:

- `^1.2.3` means `>=1.2.3, <2.0.0` → any `1.x` ≥ 1.2.3 is acceptable
- `^0.2.0` means `>=0.2.0, <0.3.0` → only `0.2.x` (the `0.x` rule is stricter: 
   minor is the breaking boundary)

So concretely:

What two crates require	      | What Cargo does
------------------------------|-----------------
serde `^1.0` and serde `^1.5` |	one serde — the highest `1.x` that satisfies both (e.g. `1.0.229`) 
serde `^1.0 `and serde `^0.9` |	two serdes — `1.0.229` and `0.9.34`, both compiled
serde `^1.0` and serde `^2.0` |	two serdes — `1.x` and `2.x`, both compiled


The dependency resolver produces a graph of `(crate_name, crate_version)` 
nodes. Cargo then:

1. **Downloads**   
   one source tarball per *distinct* version node (unpacked into 
   `~/.cargo/registry/src/…`). One version → one tarball; two versions → two 
   tarballs.

2. **Compiles**  
   each distinct downloaded tarball once, as a fully separate crate.

The key enabler: in Rust, a crate is identified by **name + version** in its 
metadata, so `serde 1.0.229` and `serde 2.0.0` are considered to be two 
*different* crates to the compiler. Their types are distinct — 
`serde1::Deserialize` and `serde2::Deserialize` are unrelated traits. They can 
coexist in the same binary because the linker resolves every reference to a 
specific `(name, version)`, so there's zero ambiguity.

The cost of duplication is extra compile time and binary size (the classic 
_"dependency bloat"_). You can see which dependencies have been unified and 
which have been duplicated with `cargo tree -d`

Also relevant: `Cargo.lock` pins the *exact* resolution so everyone builds 
the same versions; `cargo update` recomputes it.








