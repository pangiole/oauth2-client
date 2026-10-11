# Prerequisites
Our client application requires a few dependencies, namely a few popular Cargo 
crates, and a configuration file. 

## Dependencies

Our client application depends on the 
[`openidconnect`](https://crates.io/crates/openidconnect) crate (which 
in turn is built on the [`oauth2`](https://crates.io/crates/oauth2)), the 
[`axum`](https://crates.io/crates/axum) crate for the callback endpoint, and the
[`tokio`](https://crates.io/crates/tokio) crate for the asynchronous runtime. 
It also depends on the [`anyhow`](https://crates.io/crates/anyhow) crate for 
easy idiomatic error handling.

Although the reader is not required to be an expert with the above crates, a 
certain degree of familiarity helps with a deeper understanding of this small book.


## Configuration
Our application needs to be configured with settings that could change across
deployments and that could depend on the particular IdP - Identity Provider in
use.

```rust
// file: src/config.rs
// -------------------

pub enum IssuerType {
    Google, GitHub, Auth0, Keycloak, LinkedIn,
}

pub struct AppConfig {
    pub issuer_url: String,
    pub issuer_type: IssuerType,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
}
```

Identity providers may differ in how they hand out refresh tokens, so our
application needs to know *which* provider it is targeting via the
`OIDC_ISSUER_TYPE` configuration setting.

### String Representation Pattern
In general programming, providing an explicit method to convert an internal 
domain representation (like an enum variant or struct instance) into its string 
domain representation is known as the _"Stringifier Pattern"_.

Unlike implementing the 
[`ToString`](https://doc.rust-lang.org/std/string/trait.ToString.html)
trait or standard string conversions which typically allocate a new heap string,
the conventional `as_str()` method explicitly promises a zero-copy reference 
view, as it returns a string slice `&str`.

```rust
// file: src/config.rs
// -------------------

impl IssuerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssuerType::Google   => "google",
            IssuerType::GitHub   => "github",
            IssuerType::Auth0    => "auth0",
            IssuerType::Keycloak => "keycloak",
            IssuerType::LinkedIn => "linkedin",
        }
    }
}
```

Once the conventional `as_str()` method is in place, the most obvious trait we
should implement immediately after is the `Display` trait:

```rust
impl std::fmt::Display for IssuerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
```

The natural counterpart to `as_str()` is parsing a string back into an enum.
This is usually done by implementing `std::str::FromStr` trait (or the 
`TryFrom<&str>` trait):

```rust
impl std::str::FromStr for IssuerType {
    type Err = anyhow::Error;

    /// Parses the issuer type case-insensitively.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "google"   => Ok(IssuerType::Google),
            "github"   => Ok(IssuerType::GitHub),
            "auth0"    => Ok(IssuerType::Auth0),
            "keycloak" => Ok(IssuerType::Keycloak),
            "linkedin" => Ok(IssuerType::LinkedIn),
            other => Err(anyhow!(
                "unknown OIDC_ISSUER_TYPE value '{other}'; expected one of: \
                 google, github, auth0, keycloak, linkedin"
            )),
        }
    }
}
```

### Thread-Safe Lazy Singleton Pattern

Our `config::load()` function loads and validates our application configuration,
then stashes the result in a process-wide `OnceLock<AppConfig>`, using the 
_"Thread-Safe Lazy Singleton"_ pattern, also known as _"Lazy Initialization"_. 
In doing that, any other module can call the `config::get()` to obtain a 
read-only reference to the loaded configuration.

```rust 
// file: src/config.rs
// -------------------

use std::sync::OnceLock;
use anyhow::{anyhow, Result};

// pub enum IssuerType ...
// pub struct AppConfig ...

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

pub fn load() -> Result<()> {
    // Load the configuration settings from somewhere
    let loaded_config: AppConfig = ...;
    
    // Set the loaded config into the global cell
    CONFIG
      .set(loaded_config)
      .map_err(|_| anyhow!("AppConfig is already initialized"))
}

/// Returns the process-wide, read-only configuration.
pub fn get() -> &'static AppConfig {
    CONFIG
      .get()
      .expect("AppConfig not initialized; call load() first")
}
```

When declared as a top-level item, the `static` keyword creates a single memory 
location that lives for the entire duration of the program. Unlike `const`,
which inlines its value wherever it is used (creating copies in code), a static
item occupies a fixed, unique location in RAM. Passing a reference to a static 
passes an actual pointer to that exact memory address. 

The initial value of a static item **must** be known at compile time, or lazily 
initialized using the `OnceLock<T>`. This kind of lock provides a thread-safe 
**synchronization primitive** that allows an item to be initialized exactly once 
upon its first access at runtime.


### Environment variables
Following the [12-Factor App](https://www.12factor.net/config) recommendations,
our application configuration comes from the process environment, whose 
variables are loaded using the [`dotenvy`](https://crates.io/crates/dotenvy) 
crate.

```sh
# The relevant environment variables

OIDC_ISSUER_TYPE=Google
OIDC_ISSUER_URL=https://accounts.google.com
OAUTH_CLIENT_ID=<your-client-id>
OAUTH_CLIENT_SECRET=<your-client-secret>
OAUTH_SCOPES=https://www.googleapis.com/auth/youtube
```

This is well different from using configuration files like `app.conf` or 
similar. Don't get confused by the opportunity of using `.env` file as 
that must be seen just as convenience for *bootstrapping* environment variables 
during local development. The `.env` is not a configuration file or a deployment
mechanism. It is not the source of truth and never put under version control.
In production, the platform (Docker, Kubernetes, Heroku, etc.) running our 
application sets real environment variables, and no `.env` file exists!

> __The shell environment is the single source of truth!__  


```rust
// file: src/config.rs
// -------------------

use anyhow::{anyhow, Context, Result};

// struct AppConfig ...

pub fn load() -> Result<()> {
    
    // A missing `.env` file is fine!
    // The variables are supposed to be already set in the process
    // environment (e.g. by the shell or a container). Only propagate 
    // genuine errors, such as a malformed file.
    
    if let Err(err) = dotenvy::dotenv()
        && !err.not_found()
    {
        return Err(anyhow::Error::new(err).context("failed to load .env file"));
    }

    // Read each environment variable ...
    
    let issuer_url = dotenvy::var("OIDC_ISSUER_URL")
        .context("OIDC_ISSUER_URL must be set as environment variable")?;

    let issuer_type = dotenvy::var("OIDC_ISSUER_TYPE")
        .context("OIDC_ISSUER_TYPE must be set as environment variable")?
        .parse::<IssuerType>()?;
    
    // ... read more here ...
    
    // Finally make the AppConfig struct value
    let loaded_config = AppConfig {
        issuer_url,
        issuer_type,
        client_id,
        client_secret,
        scopes,
    };

    // Set the loaded config into the global cell
    // ...
}
```

Also note that the `.env` file's only legitimate job is to eventually **fill in 
missing values** for local development convenience — never to override an 
explicit environment, and that the `dotenv()` function does precisely this.

