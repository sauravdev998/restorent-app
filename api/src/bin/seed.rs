//! `pnpm db:seed`: a development restaurant with enough in it to take an order.
//!
//! It goes through the same repository operations the API does rather than
//! inserting rows, so a seeded database is one the product could have produced.
//! A seed that wrote rows directly would be the one restaurant in existence
//! whose password hash, whose audit row, and whose derived settings had never
//! been through the code that makes them, which is exactly the restaurant every
//! bug would hide behind.
//!
//! Every part checks for itself before it creates itself, so running this twice
//! leaves one of each thing rather than two or an error. That matters more than
//! it sounds: the command a developer runs when something looks wrong is this
//! one, and a seed that fails the second time teaches them to reset the whole
//! database instead.
//!
//! Refuses to run outside development. The credentials it writes are in
//! `.env.example`, which means they are in the repository, which means they are
//! public.

use anyhow::{Context as _, bail};
use rust_decimal::Decimal;

use api::application::ports::PasswordHasher as _;
use api::domain::catalog::{Dish, MenuCategory};
use api::domain::country::CountryCode;
use api::domain::credentials::{EmailAddress, Password};
use api::domain::enums::{Diet, StaffRole};
use api::domain::ids::{
    DiningTableId, DishId, MenuCategoryId, RestaurantId, StaffId, TableSectionId,
};
use api::domain::session::SessionToken;
use api::infrastructure::config::{Config, Environment};
use api::infrastructure::db::repository::catalog::{DishEdit, NewDish};
use api::infrastructure::db::repository::{accounts, catalog, sessions};
use api::infrastructure::db::{Database, ScopedTx};
use api::infrastructure::passwords::Argon2Passwords;

/// The development restaurant's name.
const RESTAURANT_NAME: &str = "The Development Kitchen";

/// Where the development restaurant is, and therefore what currency, timezone,
/// language, and formatting locale it starts with.
const COUNTRY: &str = "IN";

/// One development account: who they are and what they sign in with.
///
/// Constants rather than environment variables, and the reason is the same one
/// that makes this binary refuse to run outside development: a seeded password
/// read from the environment is a password somebody eventually sets in
/// production.
struct Account {
    /// What to call them on screen.
    display_name: &'static str,
    /// The address they sign in with. Also in `.env.example`.
    email: &'static str,
    /// Their password. Also in `.env.example`, and therefore public.
    password: &'static str,
    /// Which surface they land on.
    role: StaffRole,
}

/// The admin, who owns the restaurant and is created by registration itself.
const ADMIN: Account = Account {
    display_name: "Dev Admin",
    email: "admin@example.test",
    password: "development-only-password",
    role: StaffRole::Admin,
};

/// The two accounts the thread actually runs on: somebody to take the order and
/// somebody to cook it.
const STAFF: [Account; 2] = [
    Account {
        display_name: "Dev Waiter",
        email: "waiter@example.test",
        password: "development-only-password",
        role: StaffRole::Waiter,
    },
    Account {
        display_name: "Dev Chef",
        email: "chef@example.test",
        password: "development-only-password",
        role: StaffRole::Chef,
    },
];

/// The one section, and the four tables in it.
const SECTION_NAME: &str = "Main room";

/// Four, because two is not enough to see an occupied table beside a free one
/// and eight is a wall of buttons nobody learns anything extra from.
const TABLE_LABELS: [&str; 4] = ["1", "2", "3", "4"];

/// One dish on the seeded menu.
struct SeedDish {
    /// What it is called.
    name: &'static str,
    /// What it costs, as an exact decimal.
    price: &'static str,
    /// Veg, non veg, or egg. Set on a database where the dish already exists
    /// too, because 0006 filled every existing dish as veg and a developer's
    /// fish pakora should not read as vegetarian.
    diet: Diet,
    /// Whether the kitchen can make it. One of the six is off on purpose.
    available: bool,
}

/// The menu: two categories, six dishes, one of them switched off.
///
/// The unavailable one is the point of the list rather than a detail of it. A
/// waiter's ordering screen has to show a dish the kitchen has run out of,
/// greyed and impossible to tap, so that the waiter can tell the customer; with
/// nothing switched off in development that state is drawn by nobody and
/// noticed by nobody until a real restaurant runs out of fish.
const MENU: [(&str, &[SeedDish]); 2] = [
    (
        "Starters",
        &[
            SeedDish {
                name: "Tomato soup",
                price: "180.00",
                diet: Diet::Veg,
                available: true,
            },
            SeedDish {
                name: "Paneer tikka",
                price: "320.00",
                diet: Diet::Veg,
                available: true,
            },
            SeedDish {
                name: "Fish pakora",
                price: "290.00",
                diet: Diet::NonVeg,
                available: false,
            },
        ],
    ),
    (
        "Mains",
        &[
            SeedDish {
                name: "Dal makhani",
                price: "340.00",
                diet: Diet::Veg,
                available: true,
            },
            SeedDish {
                name: "Chicken biryani",
                price: "480.50",
                diet: Diet::NonVeg,
                available: true,
            },
            SeedDish {
                name: "Butter naan",
                price: "70.00",
                diet: Diet::Veg,
                available: true,
            },
        ],
    ),
];

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

    let (restaurant_id, admin, cookie_value) = ensure_restaurant(&database).await?;

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .context("opening the seed transaction")?;

    ensure_staff(&mut tx).await?;
    let tables = ensure_floor(&mut tx).await?;
    let dishes = ensure_menu(&mut tx, admin).await?;

    tx.commit().await.context("committing the seed")?;

    report(restaurant_id, &tables, &dishes, cookie_value.as_deref());

    Ok(())
}

/// The restaurant and its admin, registered if they are not there yet.
///
/// Returns the restaurant, its admin, who every menu change is audited as, and
/// the session cookie value if one was just minted. A repeat run mints none: a
/// fresh session row every time somebody runs the seed would be the one thing
/// about this command that did duplicate.
async fn ensure_restaurant(
    database: &Database,
) -> anyhow::Result<(RestaurantId, StaffId, Option<String>)> {
    if let Some(existing) = database
        .find_staff_for_login(ADMIN.email)
        .await
        .context("checking whether the seed has already run")?
    {
        return Ok((existing.restaurant_id, existing.staff_id, None));
    }

    let email = EmailAddress::new(ADMIN.email).context("the seed admin's email address")?;
    let country = CountryCode::new(COUNTRY).context("the seed's country")?;
    let password_hash = hash(ADMIN.password).await?;

    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .context("opening the registration transaction")?;

    let admin = accounts::register(
        &mut tx,
        &accounts::Registration {
            restaurant_name: RESTAURANT_NAME,
            country: country.settings().context("the seed's country row")?,
            display_name: ADMIN.display_name,
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

    tx.commit().await.context("committing the registration")?;

    Ok((restaurant_id, admin, Some(token.cookie_value())))
}

/// The waiter and the chef, created if they are not there yet.
///
/// Matched on the address rather than the role, because a restaurant may have
/// several of each and the address is what identifies an account.
async fn ensure_staff(tx: &mut ScopedTx<'_>) -> anyhow::Result<()> {
    for account in &STAFF {
        let existing = catalog::active_staff(tx)
            .await
            .context("reading who already works here")?
            .into_iter()
            .any(|staff| staff.email.eq_ignore_ascii_case(account.email));

        if existing {
            continue;
        }

        let email = EmailAddress::new(account.email).context("a seed account's email address")?;
        let password_hash = hash(account.password).await?;

        accounts::create_staff(
            tx,
            &accounts::NewStaff {
                display_name: account.display_name,
                email: &email,
                password_hash: &password_hash,
                role: account.role,
            },
        )
        .await
        .with_context(|| format!("creating the seeded {}", account.role.as_label()))?;
    }

    Ok(())
}

/// The section and its four tables, created if they are not there yet.
async fn ensure_floor(tx: &mut ScopedTx<'_>) -> anyhow::Result<Vec<DiningTableId>> {
    let section = match catalog::live_table_sections(tx)
        .await
        .context("reading the floor's sections")?
        .into_iter()
        .find(|section| section.name == SECTION_NAME)
    {
        Some(found) => found.id,
        None => catalog::create_table_section(tx, SECTION_NAME, 1)
            .await
            .context("creating the seeded section")?,
    };

    let mut tables = Vec::with_capacity(TABLE_LABELS.len());

    for (index, label) in TABLE_LABELS.iter().enumerate() {
        let position = position_of(index);

        let existing = catalog::live_dining_tables(tx)
            .await
            .context("reading the floor's tables")?
            .into_iter()
            .find(|table| table.label == *label);

        let id = match existing {
            Some(found) => found.id,
            None => create_table(tx, section, label, position).await?,
        };

        tables.push(id);
    }

    Ok(tables)
}

/// One seeded table, seating four, in the one section.
async fn create_table(
    tx: &mut ScopedTx<'_>,
    section: TableSectionId,
    label: &str,
    position: i32,
) -> anyhow::Result<DiningTableId> {
    catalog::create_dining_table(tx, Some(section), label, Some(4), position)
        .await
        .with_context(|| format!("creating the seeded table {label}"))
}

/// The two categories and their six dishes, created if they are not there yet.
///
/// Every change goes through the same repository calls the admin screen uses,
/// audited as the seeded admin. New ones land at the end of their list, so
/// creating them in the order written here is what gives the printed order.
async fn ensure_menu(tx: &mut ScopedTx<'_>, admin: StaffId) -> anyhow::Result<Vec<DishId>> {
    let mut created = Vec::new();

    for (category_name, dishes) in &MENU {
        let category = ensure_category(tx, category_name, admin).await?;

        for dish in *dishes {
            let id = ensure_dish(tx, category, dish, admin).await?;
            created.push(id);
        }
    }

    Ok(created)
}

/// One menu category, by name.
async fn ensure_category(
    tx: &mut ScopedTx<'_>,
    name: &str,
    admin: StaffId,
) -> anyhow::Result<MenuCategoryId> {
    let existing = catalog::live_menu_categories(tx)
        .await
        .context("reading the menu's categories")?
        .into_iter()
        .find(|category: &MenuCategory| category.name == name);

    match existing {
        Some(found) => Ok(found.id),
        None => catalog::create_menu_category(tx, name, admin)
            .await
            .map(|category| category.id)
            .with_context(|| format!("creating the seeded category {name}")),
    }
}

/// One dish, by name, under its category, with the right diet marker.
///
/// An existing dish is left where it is and as available as it is, because a
/// developer may have moved or switched it on purpose. Only a wrong diet marker
/// is corrected, through the same audited edit an admin would make.
async fn ensure_dish(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
    dish: &SeedDish,
    admin: StaffId,
) -> anyhow::Result<DishId> {
    let existing = catalog::live_dishes(tx)
        .await
        .context("reading the menu's dishes")?
        .into_iter()
        .find(|existing: &Dish| existing.name == dish.name);

    if let Some(found) = existing {
        if found.diet != dish.diet {
            catalog::update_dish(
                tx,
                found.id,
                &DishEdit {
                    category_id: found.category_id,
                    name: found.name.clone(),
                    description: found.description.clone(),
                    price: found.price,
                    diet: dish.diet,
                    version: found.version,
                },
                admin,
            )
            .await
            .with_context(|| format!("setting the diet marker on {}", dish.name))?;
        }

        return Ok(found.id);
    }

    let price: Decimal = dish
        .price
        .parse()
        .with_context(|| format!("the seeded price for {}", dish.name))?;

    let created = catalog::create_dish(
        tx,
        &NewDish {
            category_id,
            name: dish.name,
            description: None,
            price,
            diet: dish.diet,
        },
        admin,
    )
    .await
    .with_context(|| format!("creating the seeded dish {}", dish.name))?;

    // Created available, the way every new dish is, and switched off through
    // the same switch a chef uses.
    if !dish.available {
        catalog::set_dish_availability(tx, created.id, false, admin)
            .await
            .with_context(|| format!("switching off the seeded dish {}", dish.name))?;
    }

    Ok(created.id)
}

/// Where the nth thing in a seeded list sits, counting from one.
///
/// A fallible conversion rather than an `as`, because `position` is a signed
/// integer in the schema and a list long enough to overflow it would wrap
/// silently into a negative order.
fn position_of(index: usize) -> i32 {
    i32::try_from(index + 1).unwrap_or(i32::MAX)
}

/// Hashes a seeded password the way registration hashes a real one.
async fn hash(password: &str) -> anyhow::Result<String> {
    let parsed = Password::new(password).context("a seed password")?;

    Argon2Passwords::new()
        .hash(parsed)
        .await
        .context("hashing a seed password")
}

/// Says what is now in the database and how to sign in to it.
fn report(
    restaurant_id: RestaurantId,
    tables: &[DiningTableId],
    dishes: &[DishId],
    cookie_value: Option<&str>,
) {
    println!("Seeded {RESTAURANT_NAME} ({restaurant_id}).");
    println!(
        "  {} tables, {} dishes in {} categories.",
        tables.len(),
        dishes.len(),
        MENU.len()
    );
    println!();
    println!("Sign in as any of these:");

    for account in std::iter::once(&ADMIN).chain(STAFF.iter()) {
        println!(
            "  {:<6} {:<20} {}",
            account.role.as_label(),
            account.email,
            account.password
        );
    }

    if let Some(cookie_value) = cookie_value {
        println!();
        println!("Or set this cookie by hand to be signed in as the admin:");
        println!("  session={cookie_value}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-17
    ///
    /// The whole reason the greyed out state on the waiter's ordering screen
    /// gets drawn at all in development. With every seeded dish available,
    /// nobody sees that state until a real kitchen runs out of something.
    #[test]
    fn exactly_one_seeded_dish_is_switched_off() {
        let unavailable = MENU
            .iter()
            .flat_map(|(_, dishes)| dishes.iter())
            .filter(|dish| !dish.available)
            .count();

        assert_eq!(
            unavailable, 1,
            "the seed has {unavailable} unavailable dishes, so the greyed state is either \
             undrawn or over represented"
        );
    }

    /// covers: AC-17
    #[test]
    fn the_seed_creates_what_the_thread_needs_to_run() {
        assert_eq!(TABLE_LABELS.len(), 4);
        assert_eq!(MENU.len(), 2);
        assert_eq!(
            MENU.iter().map(|(_, dishes)| dishes.len()).sum::<usize>(),
            6
        );
        assert_eq!(STAFF.len(), 2);
    }

    /// Two accounts sharing an address would leave the second one uncreatable,
    /// and the seed would look like it had run when it had half run.
    #[test]
    fn every_seeded_account_has_its_own_address() {
        let mut addresses: Vec<&str> = std::iter::once(&ADMIN)
            .chain(STAFF.iter())
            .map(|account| account.email)
            .collect();

        addresses.sort_unstable();
        let total = addresses.len();
        addresses.dedup();

        assert_eq!(
            addresses.len(),
            total,
            "two seeded accounts share an address"
        );
    }

    /// One of each role the thread needs, so a developer can drive both ends of
    /// it without creating anybody by hand.
    #[test]
    fn the_seed_covers_all_three_roles() {
        let roles: Vec<StaffRole> = std::iter::once(&ADMIN)
            .chain(STAFF.iter())
            .map(|account| account.role)
            .collect();

        for role in [StaffRole::Admin, StaffRole::Waiter, StaffRole::Chef] {
            assert!(roles.contains(&role), "the seed creates no {role:?}");
        }
    }

    /// Every price has to be a decimal `rust_decimal` will parse, or the seed
    /// fails partway through with half a menu written.
    #[test]
    fn every_seeded_price_is_an_exact_decimal() {
        for (_, dishes) in &MENU {
            for dish in *dishes {
                let price: Decimal = dish
                    .price
                    .parse()
                    .unwrap_or_else(|_| panic!("{} has an unparseable price", dish.name));

                assert!(price > Decimal::ZERO, "{} is priced at nothing", dish.name);
            }
        }
    }

    /// The marker is what a customer with a dietary rule reads first, so the
    /// seed has to get it right on the dishes a developer will look at.
    #[test]
    fn every_seeded_dish_carries_the_diet_it_really_is() {
        let diet_of = |name: &str| {
            MENU.iter()
                .flat_map(|(_, dishes)| dishes.iter())
                .find(|dish| dish.name == name)
                .map(|dish| dish.diet)
        };

        assert_eq!(diet_of("Fish pakora"), Some(Diet::NonVeg));
        assert_eq!(diet_of("Chicken biryani"), Some(Diet::NonVeg));
        assert_eq!(diet_of("Paneer tikka"), Some(Diet::Veg));
        assert_eq!(diet_of("Dal makhani"), Some(Diet::Veg));
    }

    /// Two dishes with the same name would make the idempotency check treat the
    /// second as already created, so the seed would quietly write five dishes.
    #[test]
    fn no_two_seeded_dishes_share_a_name() {
        let mut names: Vec<&str> = MENU
            .iter()
            .flat_map(|(_, dishes)| dishes.iter().map(|dish| dish.name))
            .collect();

        names.sort_unstable();
        let total = names.len();
        names.dedup();

        assert_eq!(names.len(), total, "two seeded dishes share a name");
    }
}
