//! What travels down a live stream to a kitchen or waiter screen.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ids::RestaurantId;

/// Which kind of thing changed.
///
/// Kept as a closed enum on purpose: a screen reacting to an event it does not
/// understand is a bug, and a typo in a string would be silent.
///
/// These variants are the closed vocabulary the live pipeline runs on, and they
/// match the entity strings in spec 0003's interface surface one for one.
/// Nothing invents a string: adding an entity means adding a variant here and a
/// row there, in the same change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EntityKind {
    /// A party sat down, moved table, or left.
    Visit,
    /// A ticket reached the kitchen or changed how far along it is.
    OrderRound,
    /// One dish on a ticket changed.
    OrderLine,
    /// A bill was opened, had lines assigned, closed, or was paid.
    Bill,
    /// A menu item changed, most often its availability.
    Dish,
    /// A table was added, renamed, or archived.
    DiningTable,
    /// A staff account changed.
    Staff,

    /// Carries no product meaning. It exists so the scaffold can prove the
    /// whole path (HTTP, then Postgres NOTIFY, then this process, then the
    /// browser) without needing a real entity, and the development only
    /// `/api/dev/notify` endpoint still uses it.
    Probe,
}

impl EntityKind {
    /// The exact string that travels in a `NOTIFY` payload for this kind.
    ///
    /// The database side of this pairing is `notify_entity_change`, which takes
    /// the string as text. This method is the only place that string is written,
    /// so a caller cannot pass one the listener does not recognise.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Visit => "visit",
            Self::OrderRound => "order_round",
            Self::OrderLine => "order_line",
            Self::Bill => "bill",
            Self::Dish => "dish",
            Self::DiningTable => "dining_table",
            Self::Staff => "staff",
            Self::Probe => "probe",
        }
    }
}

/// A change worth telling connected screens about.
///
/// It carries a kind and an id, never row content. A client that receives one
/// goes and asks for the row, and row level security decides whether it may
/// have it. Putting the row in the event would make this stream a second,
/// unguarded way to read data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Whose event this is. The server routes on this and never sends an event
    /// to a stream belonging to a different restaurant.
    pub restaurant_id: RestaurantId,
    /// What kind of thing changed.
    pub entity: EntityKind,
    /// Which one changed.
    pub entity_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind, in one place, so the tests below cover all of them.
    ///
    /// Kept honest by [`position`], the same way `domain::audit` does it.
    const ALL: [EntityKind; 8] = [
        EntityKind::Visit,
        EntityKind::OrderRound,
        EntityKind::OrderLine,
        EntityKind::Bill,
        EntityKind::Dish,
        EntityKind::DiningTable,
        EntityKind::Staff,
        EntityKind::Probe,
    ];

    /// Where each kind sits in [`ALL`].
    ///
    /// Exhaustive on purpose: a new variant fails to compile here rather than
    /// slipping past the round trip test below, which is the one place that
    /// would have caught its label and its wire name disagreeing.
    const fn position(kind: EntityKind) -> usize {
        match kind {
            EntityKind::Visit => 0,
            EntityKind::OrderRound => 1,
            EntityKind::OrderLine => 2,
            EntityKind::Bill => 3,
            EntityKind::Dish => 4,
            EntityKind::DiningTable => 5,
            EntityKind::Staff => 6,
            EntityKind::Probe => 7,
        }
    }

    #[test]
    fn the_list_the_other_tests_run_over_holds_every_kind() {
        for (index, kind) in ALL.into_iter().enumerate() {
            assert_eq!(
                position(kind),
                index,
                "{kind:?} is not where the list says it is"
            );
        }
    }

    /// covers: AC-7
    ///
    /// The two halves of the live path do not use the same mechanism. Sending
    /// writes [`EntityKind::as_label`] into the `NOTIFY` payload by hand, and
    /// receiving reads it back through serde, which uses the variant name in
    /// snake case. Nothing makes those agree except this test.
    ///
    /// A disagreement fails in the worst available way. The listener treats an
    /// unreadable payload as never fatal: it logs it and carries on, so the
    /// notification is dropped, the screen holding the stream is simply never
    /// told, and every test that does not hold a real stream open still passes.
    #[test]
    fn every_label_is_the_wire_name_the_listener_reads_back() {
        for kind in ALL {
            let label = kind.as_label();
            let on_the_wire = serde_json::to_value(kind).expect("an entity kind serialises");

            assert_eq!(
                on_the_wire,
                serde_json::Value::String(label.to_owned()),
                "{kind:?} is sent as {label:?} but travels as {on_the_wire}"
            );

            let read_back: EntityKind = serde_json::from_value(on_the_wire)
                .expect("what an entity kind serialises to deserialises again");

            assert_eq!(read_back, kind, "{kind:?} does not survive the round trip");
        }
    }

    /// covers: AC-7
    ///
    /// The payload is not built in Rust. `notify_entity_change` builds it in
    /// SQL with `json_build_object`, naming all three fields there, so renaming
    /// a field on [`DomainEvent`] would leave the migration writing one shape
    /// and this process expecting another. This is that pairing, written out as
    /// the database writes it.
    #[test]
    fn a_payload_shaped_the_way_the_database_builds_it_reads_back_whole() {
        let payload = r#"{
            "restaurant_id": "018f3f4a-0000-7000-8000-000000000001",
            "entity": "order_round",
            "entity_id": "018f3f4a-0000-7000-8000-000000000002"
        }"#;

        let event: DomainEvent =
            serde_json::from_str(payload).expect("the payload the migration builds is readable");

        assert_eq!(
            event.restaurant_id.to_string(),
            "018f3f4a-0000-7000-8000-000000000001"
        );
        assert_eq!(event.entity, EntityKind::OrderRound);
        assert_eq!(
            event.entity_id.to_string(),
            "018f3f4a-0000-7000-8000-000000000002"
        );
    }

    /// A kind this process does not know must not decode as some other kind.
    ///
    /// Dropping it is the intended behaviour, and it is safe only because the
    /// alternative is worse: a screen acting on the wrong entity is a bug a user
    /// sees, and a dropped notification is one a reconnect fixes.
    #[test]
    fn an_entity_string_this_process_does_not_know_is_refused_rather_than_guessed() {
        assert!(
            serde_json::from_str::<EntityKind>("\"table_section\"").is_err(),
            "an unknown entity string decoded to something"
        );
        assert!(
            serde_json::from_str::<EntityKind>("\"orderround\"").is_err(),
            "an entity string missing its underscore decoded to something"
        );
    }

    /// Two kinds sharing a label would route a change to the wrong screen.
    #[test]
    fn no_two_kinds_share_a_label() {
        for kind in ALL {
            let clashes = ALL
                .into_iter()
                .filter(|other| other.as_label() == kind.as_label())
                .count();

            assert_eq!(
                clashes,
                1,
                "{kind:?} shares its label {:?} with another kind",
                kind.as_label()
            );
        }
    }
}
