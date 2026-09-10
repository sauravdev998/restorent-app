//! The `OpenAPI` document, generated from the handlers themselves.
//!
//! This is the honest half of the seam between two languages. `openapi.json` is
//! generated from here, the TypeScript client is generated from that, and both
//! are committed. Continuous integration regenerates them and fails if the
//! result differs, so renaming a field in Rust breaks the pull request instead
//! of breaking a Saturday night.
//!
//! A new route must be listed in `paths` below, or it will not reach the client.

use utoipa::OpenApi;

use super::dto::{
    BillDto, BillTaxDto, IdentityBundle, LineStatusDto, OrderLineDto, OrderRoundDto, RestaurantDto,
    RoleDto, RoundStatusDto, StaffDto,
};
use super::error::ErrorBody;
use super::handlers::{auth, billing, events, health, me, menu, service};

/// The whole public API surface.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Restaurant operations platform API",
        description = "Orders at the table, tickets live in the kitchen, a bill at the end.",
        version = "0.1.0",
    ),
    paths(
        health::health,
        events::events,
        auth::register,
        auth::sign_in,
        auth::sign_out,
        auth::me,
        me::update_me,
        me::change_password,
        me::update_restaurant,
        service::floor,
        menu::menu,
        service::open_visit,
        billing::visit,
        service::send_round,
        billing::close_visit,
        service::mark_round_served,
        service::kitchen_tickets,
        service::mark_line_ready,
    ),
    components(schemas(
        ErrorBody,
        health::HealthResponse,
        health::Component,
        events::StreamEvent,
        IdentityBundle,
        StaffDto,
        RestaurantDto,
        RoleDto,
        auth::RegisterRequest,
        auth::SignInRequest,
        me::UpdateMeRequest,
        me::ChangePasswordRequest,
        me::UpdateRestaurantRequest,
        LineStatusDto,
        RoundStatusDto,
        OrderLineDto,
        OrderRoundDto,
        BillTaxDto,
        BillDto,
        menu::MenuResponse,
        menu::MenuCategoryDto,
        menu::MenuDishDto,
        service::FloorResponse,
        service::FloorSectionDto,
        service::FloorTableDto,
        service::OccupancyDto,
        service::OpenVisitRequest,
        service::OpenVisitResponse,
        service::SendRoundRequest,
        service::SendRoundLine,
        service::KitchenResponse,
        service::KitchenTicketDto,
        service::KitchenLineDto,
        service::MarkedLineResponse,
        billing::VisitResponse,
    )),
    tags(
        (name = "system", description = "Health and live updates."),
        (name = "accounts", description = "Registering, signing in, and who is signed in."),
        (name = "orders", description = "The floor, the menu, tickets to the kitchen, and the bill."),
    )
)]
pub struct ApiDoc;

/// The document as pretty JSON, for `openapi.json`.
///
/// # Errors
///
/// Returns an error only if the document cannot be serialised, which would mean
/// a broken schema annotation.
pub fn to_pretty_json() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&ApiDoc::openapi())
}
