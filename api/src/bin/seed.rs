//! `pnpm db:seed`: one restaurant and one admin, for development.
//!
//! It registers through the same code path the API does rather than inserting
//! rows, so a seeded database is one the product could have produced. A seed
//! that wrote rows directly would be the one restaurant in existence whose
//! password hash, whose audit row, and whose derived settings had never been
//! through the code that makes them, which is exactly the restaurant every bug
//! would hide behind.
//!
//! Refuses to run outside development. The credentials it writes are in
//! `.env.example`, which means they are in the repository, which means they are
//! public.

use anyhow::{Context as _, bail};

use api::application::ports::PasswordHasher as _;
use api::domain::country::CountryCode;
use api::domain::credentials::{EmailAddress, Password};
use api::domain::ids::RestaurantId;
use api::domain::session::SessionToken;
use api::infrastructure::config::{Config, Environment};
use api::infrastructure::db::Database;
use api::infrastructure::db::repository::{accounts, sessions};
use api::infrastructure::passwords::Argon2Passwords;

/// The development restaurant's name.
const RESTAURANT_NAME: &str = "The Development Kitchen";

/// The development admin's name.
const DISPLAY_NAME: &str = "Dev Admin";

/// The development admin's address. Also in `.env.example`.
const EMAIL: &str = "admin@example.test";

/// The development admin's password. Also in `.env.example`, and therefore
/// public. That is why this binary refuses to run outside development.
const PASSWORD: &str = "development-only-password";

/// Where the development restaurant is, and therefore what currency, timezone,
/// language, and formatting locale it starts with.
const COUNTRY: &str = "IN";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let config = Config::from_env().context("configuration is not usable")?;

    if config.environment != Environment::Development {
        bail!(
            "the seed writes credentials that are published in .env.example, so it refuses to \
             run outside development"
        );
    }

    let database = Database::connect(&config)
        .await
        .context("could not open the database connection pool")?;

    let email = EmailAddress::new(EMAIL).context("the seed's email address")?;
    let password = Password::new(PASSWORD).context("the seed's password")?;
    let country = CountryCode::new(COUNTRY).context("the seed's country")?;

    if database
        .find_staff_for_login(EMAIL)
        .await
        .context("checking whether the seed has already run")?
        .is_some()
    {
        println!("The seeded admin already exists. Nothing to do.");
        println!("  email:    {EMAIL}");
        println!("  password: {PASSWORD}");
        return Ok(());
    }

    let password_hash = Argon2Passwords::new()
        .hash(password)
        .await
        .context("hashing the seed's password")?;

    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .context("opening the seed transaction")?;

    let admin = accounts::register(
        &mut tx,
        &accounts::Registration {
            restaurant_name: RESTAURANT_NAME,
            country: country.settings().context("the seed's country row")?,
            display_name: DISPLAY_NAME,
            email: &email,
            password_hash: &password_hash,
        },
    )
    .await
    .context("registering the seed restaurant")?;

    // A session too, so a browser pointed at the development API is signed in
    // without anybody typing anything. The token is printed rather than stored:
    // it is the one and only time it exists outside a cookie.
    let token = SessionToken::mint().context("minting the seed session")?;
    sessions::open(&mut tx, admin, &token.hash())
        .await
        .context("opening the seed session")?;

    tx.commit().await.context("committing the seed")?;

    println!("Seeded one restaurant and one admin.");
    println!("  restaurant: {RESTAURANT_NAME} ({restaurant_id})");
    println!("  email:      {EMAIL}");
    println!("  password:   {PASSWORD}");
    println!();
    println!("Sign in at the web app, or set this cookie by hand:");
    println!("  session={}", token.cookie_value());

    Ok(())
}
