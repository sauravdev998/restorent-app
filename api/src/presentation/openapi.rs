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
    OrderRoundDto, RestaurantDto, RoleDto, RoundStatusDto, StaffDto, StaffMemberDto, VoidReasonDto,
};
use super::error::ErrorBody;
use super::handlers::{
    admin_floor, admin_menu, auth, billing, events, health, me, menu, service, staff,
};

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
        service::open_orders,
        service::take_over,
        service::move_visit,
        service::mark_line_served,
        service::void_line,
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
        admin_floor::admin_floor,
        admin_floor::create_section,
        admin_floor::rename_section,
        admin_floor::reorder_sections,
        admin_floor::archive_section,
        admin_floor::restore_section,
        admin_floor::create_table,
        admin_floor::create_table_range,
        admin_floor::edit_table,
        admin_floor::reorder_tables,
        admin_floor::archive_table,
        admin_floor::restore_table,
        staff::list_staff,
        staff::create_staff,
        staff::edit_staff,
        staff::change_role,
        staff::reset_password,
        staff::deactivate,
        staff::reactivate,
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
        VoidReasonDto,
        service::OpenOrdersResponse,
        service::OpenOrderDto,
        service::TakeOverRequest,
        service::TakeOverResponse,
        service::MoveVisitRequest,
        service::MoveVisitResponse,
        service::VoidLineRequest,
        service::VoidLineResponse,
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
        admin_floor::AdminFloorResponse,
        admin_floor::AdminFloorGroupDto,
        admin_floor::AdminTableDto,
        admin_floor::ArchivedFloorDto,
        admin_floor::ArchivedSectionDto,
        admin_floor::ArchivedSectionTableDto,
        admin_floor::ArchivedTableDto,
        admin_floor::SectionDto,
        admin_floor::TableDto,
        admin_floor::CreateSectionRequest,
        admin_floor::RenameSectionRequest,
        admin_floor::SectionOrderRequest,
        admin_floor::RestoreSectionRequest,
        admin_floor::RestoredSectionDto,
        admin_floor::CreateTableRequest,
        admin_floor::CreateTableRangeRequest,
        admin_floor::EditTableRequest,
        admin_floor::TableOrderRequest,
        admin_floor::RestoreTableRequest,
        StaffMemberDto,
        staff::StaffListResponse,
        staff::CreateStaffRequest,
        staff::EditStaffRequest,
        staff::ChangeRoleRequest,
        staff::ResetPasswordRequest,
    )),
    tags(
        (name = "system", description = "Health and live updates."),
        (name = "accounts", description = "Registering, signing in, and who is signed in."),
        (name = "orders", description = "The floor, the menu, tickets to the kitchen, and the bill."),
        (name = "menu", description = "Building the menu, and switching a dish on or off."),
        (name = "floor", description = "Building the floor: sections, tables, their order, and what was removed."),
        (name = "staff", description = "Who works here, and the six things an admin does to an account."),
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

    /// covers: AC-16 (spec 0010)
    ///
    /// The same check for the floor: every admin floor endpoint is in the
    /// document and says it is admin only, and the waiter's floor still says it
    /// is the waiter's.
    #[test]
    fn every_floor_endpoint_is_in_the_document_and_says_who_may_call_it() {
        let document = serde_json::to_value(ApiDoc::openapi()).expect("the document serialises");

        let expected = [
            ("/api/admin/floor", "get", "Admins only."),
            ("/api/admin/floor/sections", "post", "Admins only."),
            ("/api/admin/floor/sections/order", "put", "Admins only."),
            ("/api/admin/floor/sections/{id}", "put", "Admins only."),
            (
                "/api/admin/floor/sections/{id}/archive",
                "post",
                "Admins only.",
            ),
            (
                "/api/admin/floor/sections/{id}/restore",
                "post",
                "Admins only.",
            ),
            ("/api/admin/floor/tables", "post", "Admins only."),
            ("/api/admin/floor/tables/range", "post", "Admins only."),
            ("/api/admin/floor/table-order", "put", "Admins only."),
            ("/api/admin/floor/tables/{id}", "put", "Admins only."),
            (
                "/api/admin/floor/tables/{id}/archive",
                "post",
                "Admins only.",
            ),
            (
                "/api/admin/floor/tables/{id}/restore",
                "post",
                "Admins only.",
            ),
            ("/api/floor", "get", "Waiters only."),
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

    /// covers: AC-15
    ///
    /// The same check for the seven staff endpoints. A route missing from
    /// `paths` is missing from the typed client with nothing failing, and one
    /// that does not say who holds it leaves whoever builds the screen to guess.
    #[test]
    fn every_staff_endpoint_is_in_the_document_and_says_it_is_admin_only() {
        let document = serde_json::to_value(ApiDoc::openapi()).expect("the document serialises");

        let expected = [
            ("/api/staff", "get"),
            ("/api/staff", "post"),
            ("/api/staff/{id}", "patch"),
            ("/api/staff/{id}/role", "put"),
            ("/api/staff/{id}/password", "post"),
            ("/api/staff/{id}/deactivate", "post"),
            ("/api/staff/{id}/reactivate", "post"),
        ];

        for (path, method) in expected {
            let operation = &document["paths"][path][method];
            assert!(
                operation.is_object(),
                "{method} {path} is missing from the OpenAPI document"
            );

            let responses = operation["responses"].to_string();
            assert!(
                responses.contains("Admins only."),
                "{method} {path} does not say it is admin only in its responses"
            );
            assert!(
                responses.contains("\"403\"") && responses.contains("\"401\""),
                "{method} {path} does not document its 401 and 403"
            );
        }
    }

    /// covers: AC-18 (spec 0011)
    ///
    /// Every endpoint the waiter service flow adds or changes is in the
    /// document, says it is the waiter's, and documents its `401` and `403`.
    #[test]
    fn every_waiter_service_endpoint_is_in_the_document_and_says_it_is_the_waiters() {
        let document = serde_json::to_value(ApiDoc::openapi()).expect("the document serialises");

        let expected = [
            ("/api/orders/open", "get"),
            ("/api/floor", "get"),
            ("/api/visits/{id}", "get"),
            ("/api/visits/{id}/rounds", "post"),
            ("/api/visits/{id}/take-over", "post"),
            ("/api/visits/{id}/move", "post"),
            ("/api/visits/{id}/close", "post"),
            ("/api/order-lines/{id}/served", "post"),
            ("/api/order-lines/{id}/void", "post"),
            ("/api/rounds/{id}/served", "post"),
        ];

        for (path, method) in expected {
            let operation = &document["paths"][path][method];
            assert!(
                operation.is_object(),
                "{method} {path} is missing from the OpenAPI document"
            );

            let responses = operation["responses"].to_string();
            assert!(
                responses.contains("Waiters only."),
                "{method} {path} does not say it is the waiter's in its responses"
            );
            assert!(
                responses.contains("\"403\"") && responses.contains("\"401\""),
                "{method} {path} does not document its 401 and 403"
            );
        }
    }
}
