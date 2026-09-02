//! Shared harness for the integration tests.
//!
//! Two things about it are load bearing.
//!
//! **It connects as `app_api`, never as the schema owner.** Postgres exempts a
//! table's owner from its own policies unless `FORCE ROW LEVEL SECURITY` is set,
//! and half of what these tests prove is that the policies apply. A suite run as
//! the owner would pass while proving nothing, which is the worst possible
//! outcome for an isolation test.
//!
//! **Most tests never commit.** A test opens a scoped transaction, seeds into
//! it, asserts, and drops it, so the database is left exactly as it was found.
//! The exceptions are the concurrency tests, which need two connections to see
//! each other's work and therefore have to commit; they clean up after
//! themselves by deleting the restaurant, which cascades.

// Each integration test binary compiles this whole module but uses only part of
// it, so unused warnings here are an artefact of how Cargo builds test crates
// rather than anything real. Scoped to this test only module, never to the crate.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::str::FromStr;

use rust_decimal::Decimal;
use uuid::Uuid;

use api::domain::ids::{
    DiningTableId, DishId, MenuCategoryId, RestaurantId, StaffId, TableSectionId, TaxComponentId,
};
use api::infrastructure::config::{Config, Environment};
use api::infrastructure::db::{Database, ScopedTx};

/// Opens a pool as the API role.
///
/// Reads `DATABASE_URL` the same way the real process does, falling back to the
/// repository's `.env` so `cargo test` works without anything being exported by
/// hand.
pub async fn database() -> Database {
    // The api crate's directory is the working directory for its tests, and the
    // .env lives one level up at the repository root.
    let _ = dotenvy::from_path("../.env");

    let database_url = std::env::var("DATABASE_URL").expect(
        "DATABASE_URL must be set for the integration tests. \
         Run `pnpm db:up && pnpm migrate` first.",
    );

    let config = Config {
        database_url,
        database_max_connections: 8,
        bind_address: SocketAddr::from(([127, 0, 0, 1], 0)),
        environment: Environment::Development,
    };

    Database::connect(&config)
        .await
        .expect("the integration tests need a reachable database")
}

/// A decimal written as a literal in a test.
pub fn money(value: &str) -> Decimal {
    Decimal::from_str(value).expect("test constant is a valid decimal")
}

/// One restaurant set up far enough to serve somebody.
pub struct Fixture {
    /// Which restaurant this is.
    pub restaurant_id: RestaurantId,
    /// An admin, for edits that need an actor.
    pub admin: StaffId,
    /// A waiter, who opens visits and sends tickets.
    pub waiter: StaffId,
    /// A chef, who marks dishes ready.
    pub chef: StaffId,
    /// The one section.
    pub section: TableSectionId,
    /// A table.
    pub table_one: DiningTableId,
    /// Another table, for move and occupancy tests.
    pub table_two: DiningTableId,
    /// The one menu category.
    pub category: MenuCategoryId,
    /// Priced at 9.5000.
    pub soup: DishId,
    /// Priced at 24.9950, so a subtotal has something to round.
    pub steak: DishId,
    /// Value added tax at 20%.
    pub vat: TaxComponentId,
    /// The email the admin signs in with.
    pub admin_email: String,
}

/// Seeds a whole restaurant into an already scoped transaction.
///
/// Written as raw statements rather than through the repository on purpose:
/// registering a restaurant and creating staff belong to feature 7, and a test
/// harness that invented those paths now would be inventing the very thing that
/// feature has to decide.
pub async fn seed(tx: &mut ScopedTx<'_>, restaurant_id: RestaurantId) -> Fixture {
    seed_with(
        tx,
        restaurant_id,
        "EUR",
        2,
        "Europe/Berlin",
        Some(money("12.5")),
    )
    .await
}

/// The same, for a restaurant with its own currency, timezone, or service
/// charge.
pub async fn seed_with(
    tx: &mut ScopedTx<'_>,
    restaurant_id: RestaurantId,
    currency_code: &str,
    currency_decimals: i16,
    timezone: &str,
    service_charge_percent: Option<Decimal>,
) -> Fixture {
    let raw = restaurant_id.as_uuid();
    let suffix = raw.simple().to_string();

    sqlx::query(
        "INSERT INTO restaurants
             (id, name, currency_code, currency_decimals, timezone,
              service_charge_percent, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, now())",
    )
    .bind(raw)
    .bind(format!("Test Restaurant {suffix}"))
    .bind(currency_code)
    .bind(currency_decimals)
    .bind(timezone)
    .bind(service_charge_percent)
    .execute(tx.connection())
    .await
    .expect("seeding the restaurant");

    let admin_email = format!("admin-{suffix}@example.test");
    let admin = seed_staff(tx, raw, &admin_email, "Ada Admin", "admin").await;
    let waiter = seed_staff(
        tx,
        raw,
        &format!("waiter-{suffix}@example.test"),
        "Wes Waiter",
        "waiter",
    )
    .await;
    let chef = seed_staff(
        tx,
        raw,
        &format!("chef-{suffix}@example.test"),
        "Cleo Chef",
        "chef",
    )
    .await;

    let section = TableSectionId::new();
    sqlx::query(
        "INSERT INTO table_sections (id, restaurant_id, name, position, updated_at)
         VALUES ($1, $2, 'Terrace', 1, now())",
    )
    .bind(section.as_uuid())
    .bind(raw)
    .execute(tx.connection())
    .await
    .expect("seeding the section");

    let table_one = seed_table(tx, raw, section, "T1", 1).await;
    let table_two = seed_table(tx, raw, section, "T2", 2).await;

    let category = MenuCategoryId::new();
    sqlx::query(
        "INSERT INTO menu_categories (id, restaurant_id, name, position, updated_at)
         VALUES ($1, $2, 'Mains', 1, now())",
    )
    .bind(category.as_uuid())
    .bind(raw)
    .execute(tx.connection())
    .await
    .expect("seeding the category");

    let soup = seed_dish(tx, raw, category, "Soup", money("9.5000"), 1).await;
    let steak = seed_dish(tx, raw, category, "Steak", money("24.9950"), 2).await;

    let vat = TaxComponentId::new();
    sqlx::query(
        "INSERT INTO tax_components
             (id, restaurant_id, name, rate_percent, position, updated_at)
         VALUES ($1, $2, 'VAT', 20.000, 1, now())",
    )
    .bind(vat.as_uuid())
    .bind(raw)
    .execute(tx.connection())
    .await
    .expect("seeding the tax component");

    Fixture {
        restaurant_id,
        admin,
        waiter,
        chef,
        section,
        table_one,
        table_two,
        category,
        soup,
        steak,
        vat,
        admin_email,
    }
}

async fn seed_staff(
    tx: &mut ScopedTx<'_>,
    restaurant_id: Uuid,
    email: &str,
    display_name: &str,
    role: &str,
) -> StaffId {
    let id = StaffId::new();

    sqlx::query(
        "INSERT INTO staff
             (id, restaurant_id, email, password_hash, display_name, role, updated_at)
         VALUES ($1, $2, $3, 'not-a-real-hash', $4, $5::staff_role, now())",
    )
    .bind(id.as_uuid())
    .bind(restaurant_id)
    .bind(email)
    .bind(display_name)
    .bind(role)
    .execute(tx.connection())
    .await
    .expect("seeding a staff member");

    id
}

async fn seed_table(
    tx: &mut ScopedTx<'_>,
    restaurant_id: Uuid,
    section: TableSectionId,
    label: &str,
    position: i32,
) -> DiningTableId {
    let id = DiningTableId::new();

    sqlx::query(
        "INSERT INTO dining_tables
             (id, restaurant_id, section_id, label, seats, position, updated_at)
         VALUES ($1, $2, $3, $4, 4, $5, now())",
    )
    .bind(id.as_uuid())
    .bind(restaurant_id)
    .bind(section.as_uuid())
    .bind(label)
    .bind(position)
    .execute(tx.connection())
    .await
    .expect("seeding a table");

    id
}

async fn seed_dish(
    tx: &mut ScopedTx<'_>,
    restaurant_id: Uuid,
    category: MenuCategoryId,
    name: &str,
    price: Decimal,
    position: i32,
) -> DishId {
    let id = DishId::new();

    sqlx::query(
        "INSERT INTO dishes
             (id, restaurant_id, category_id, name, price, is_available, position, updated_at)
         VALUES ($1, $2, $3, $4, $5, true, $6, now())",
    )
    .bind(id.as_uuid())
    .bind(restaurant_id)
    .bind(category.as_uuid())
    .bind(name)
    .bind(price)
    .bind(position)
    .execute(tx.connection())
    .await
    .expect("seeding a dish");

    id
}

/// Re points an open transaction at a different restaurant.
///
/// A deliberate back door, and only a test has any business using it: real code
/// gets its scope from `begin_scoped` and never changes it. Tests need it to
/// seed a second restaurant's rows and then look at them from the first
/// restaurant's point of view, all inside one transaction that rolls back.
///
/// Always leave the transaction scoped to the restaurant it was opened for
/// before calling anything from the repository, or its inserts will fail the
/// policy's check.
pub async fn rescope(tx: &mut ScopedTx<'_>, restaurant_id: RestaurantId) {
    sqlx::query("SELECT set_config('app.restaurant_id', $1, true)")
        .bind(restaurant_id.to_string())
        .execute(tx.connection())
        .await
        .expect("re scoping the transaction");
}

/// Seeds a visit and an open bill with raw statements, naming the restaurant
/// explicitly.
///
/// Needed for the second restaurant in a cross tenant test. The repository
/// cannot be used there: it takes the restaurant from the transaction handle,
/// which [`rescope`] deliberately does not move, so its inserts would name one
/// restaurant while the policy checked another.
pub async fn seed_visit_and_bill(
    tx: &mut ScopedTx<'_>,
    restaurant_id: RestaurantId,
    table_id: DiningTableId,
    staff_id: StaffId,
) -> (Uuid, Uuid) {
    let visit_id = Uuid::now_v7();
    let bill_id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO visits
             (id, restaurant_id, table_id, status, opened_by_staff_id, opened_at, updated_at)
         VALUES ($1, $2, $3, 'open', $4, now(), now())",
    )
    .bind(visit_id)
    .bind(restaurant_id.as_uuid())
    .bind(table_id.as_uuid())
    .bind(staff_id.as_uuid())
    .execute(tx.connection())
    .await
    .expect("seeding a visit");

    sqlx::query(
        "INSERT INTO bills
             (id, restaurant_id, visit_id, status, currency_code, currency_decimals,
              opened_by_staff_id, updated_at)
         VALUES ($1, $2, $3, 'open', 'EUR', 2, $4, now())",
    )
    .bind(bill_id)
    .bind(restaurant_id.as_uuid())
    .bind(visit_id)
    .bind(staff_id.as_uuid())
    .execute(tx.connection())
    .await
    .expect("seeding a bill");

    (visit_id, bill_id)
}

/// Marks a point the transaction can be rewound to.
///
/// A statement that fails aborts the whole transaction, so a test that expects a
/// refusal and then wants to carry on needs one of these around it.
pub async fn savepoint(tx: &mut ScopedTx<'_>, name: &'static str) {
    sqlx::query(sqlx::AssertSqlSafe(format!("SAVEPOINT {name}")))
        .execute(tx.connection())
        .await
        .expect("setting a savepoint");
}

/// Rewinds to a savepoint, undoing the failed statement and reviving the
/// transaction.
pub async fn rollback_to(tx: &mut ScopedTx<'_>, name: &'static str) {
    sqlx::query(sqlx::AssertSqlSafe(format!("ROLLBACK TO SAVEPOINT {name}")))
        .execute(tx.connection())
        .await
        .expect("rewinding to a savepoint");
}

/// Unsets the scope entirely, to prove an unscoped transaction sees nothing.
pub async fn unscope(tx: &mut ScopedTx<'_>) {
    sqlx::query("SELECT set_config('app.restaurant_id', '', true)")
        .execute(tx.connection())
        .await
        .expect("clearing the scope");
}

/// How many rows of a table this transaction can currently see.
pub async fn visible_rows(tx: &mut ScopedTx<'_>, table: &str) -> i64 {
    // The table name comes from the fixed list in the isolation test, never from
    // anything outside this file.
    let statement = format!("SELECT count(*) FROM {table}");

    let (count,): (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(statement))
        .fetch_one(tx.connection())
        .await
        .expect("counting visible rows");

    count
}

/// Deletes a restaurant and, by cascade, everything belonging to it.
///
/// Used by the tests that have to commit. One statement is enough because every
/// tenant table's restaurant foreign key cascades.
pub async fn drop_restaurant(database: &Database, restaurant_id: RestaurantId) {
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening a transaction to clean up");

    sqlx::query("DELETE FROM restaurants WHERE id = $1")
        .bind(restaurant_id.as_uuid())
        .execute(tx.connection())
        .await
        .expect("cleaning up the restaurant");

    tx.commit().await.expect("committing the clean up");
}

/// Every table that carries a `restaurant_id` and is therefore tenant scoped.
pub const TENANT_TABLES: &[&str] = &[
    "restaurants",
    "tax_components",
    "staff",
    "sessions",
    "table_sections",
    "dining_tables",
    "menu_categories",
    "dishes",
    "visits",
    "order_rounds",
    "bills",
    "order_lines",
    "bill_taxes",
    "payments",
    "bill_number_counters",
    "audit_log",
];
