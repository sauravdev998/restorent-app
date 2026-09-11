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
    BillDto, BillTaxDto, DietDto, DishDto, IdentityBundle, LineStatusDto, OrderLineDto,
    OrderRoundDto, RestaurantDto, RoleDto, RoundStatusDto, StaffDto,
};
use super::error::ErrorBody;
use super::handlers::{admin_menu, auth, billing, events, health, me, menu, service};

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
        admin_menu::admin_menu,
        admin_menu::create_category,
        admin_menu::rename_category,
        admin_menu::reorder_categories,
        admin_menu::reorder_dishes,
        admin_menu::archive_category,
        admin_menu::restore_category,
        admin_menu::create_dish,
        admin_menu::edit_dish,
        admin_menu::archive_dish,
        admin_menu::restore_dish,
        menu::set_availability,
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
        DietDto,
        DishDto,
        menu::SetAvailabilityRequest,
        admin_menu::AdminMenuResponse,
        admin_menu::AdminCategoryDto,
        admin_menu::ArchivedMenuDto,
        admin_menu::ArchivedCategoryDto,
        admin_menu::ArchivedDishDto,
        admin_menu::CategoryDto,
        admin_menu::CreateDishRequest,
        admin_menu::CreateCategoryRequest,
        admin_menu::RenameCategoryRequest,
        admin_menu::EditDishRequest,
        admin_menu::ReorderRequest,
        admin_menu::RestoreDishRequest,
    )),
    tags(
        (name = "system", description = "Health and live updates."),
        (name = "accounts", description = "Registering, signing in, and who is signed in."),
        (name = "orders", description = "The floor, the menu, tickets to the kitchen, and the bill."),
        (name = "menu", description = "Building the menu, and switching a dish on or off."),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-16 (spec 0008)
    ///
    /// A route missing from `paths` is missing from the typed client with
    /// nothing failing, so each menu endpoint is named here with the role its
    /// success response says holds it. The role is what a person building a
    /// screen reads to know who the endpoint is for.
    #[test]
    fn every_menu_endpoint_is_in_the_document_and_says_who_may_call_it() {
        let document = serde_json::to_value(ApiDoc::openapi()).expect("the document serialises");

        let expected = [
            ("/api/admin/menu", "get", "Admins only."),
            ("/api/admin/menu/categories", "post", "Admins only."),
            ("/api/admin/menu/categories/order", "put", "Admins only."),
            ("/api/admin/menu/categories/{id}", "put", "Admins only."),
            (
                "/api/admin/menu/categories/{id}/archive",
                "post",
                "Admins only.",
            ),
            (
                "/api/admin/menu/categories/{id}/restore",
                "post",
                "Admins only.",
            ),
            (
                "/api/admin/menu/categories/{id}/dish-order",
                "put",
                "Admins only.",
            ),
            ("/api/admin/menu/dishes", "post", "Admins only."),
            ("/api/admin/menu/dishes/{id}", "put", "Admins only."),
            (
                "/api/admin/menu/dishes/{id}/archive",
                "post",
                "Admins only.",
            ),
            (
                "/api/admin/menu/dishes/{id}/restore",
                "post",
                "Admins only.",
            ),
            ("/api/dishes/{id}/availability", "put", "Admins and chefs."),
            ("/api/menu", "get", "Waiters and chefs."),
        ];

        for (path, method, role) in expected {
            let operation = &document["paths"][path][method];
            assert!(
                operation.is_object(),
                "{method} {path} is missing from the OpenAPI document"
            );

            let responses = operation["responses"].to_string();
            assert!(
                responses.contains(role),
                "{method} {path} does not say {role:?} in its responses"
            );
            assert!(
                responses.contains("\"403\"") && responses.contains("\"401\""),
                "{method} {path} does not document its 401 and 403"
            );
        }
    }
}
