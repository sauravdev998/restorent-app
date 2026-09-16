# 0009. Staff accounts: rationale

Reasoning, options, and the forces behind [index.md](index.md).

## Context

Spec 0006 built exactly one way for a person to exist on this platform: somebody registers a
restaurant and becomes its admin. There is no second path. A restaurant that hires a waiter today
cannot give them an account at all, which means every screen feature 12 and feature 13 build has
nobody but the owner to sign in as. This is the last gap between the product and a real restaurant
using it on a real evening.

Three forces shape the answer, and all three come from decisions already taken.

**There is no email provider, deliberately.** Spec 0006 refused to take one on, and feature 24
(password recovery by email) is the feature that would bring one in. So the credential a new member
of staff first uses has to travel from the admin to that person by voice or by paper, not by inbox.
That is a weaker channel than an emailed link, and whatever is designed here has to contain the
weakness rather than pretend it is not there.

**Spec 0006 already wrote three of this feature's rules and left them unbuilt.** Its state
transitions table says that deactivating an account, changing a role, and an admin resetting
somebody's password each revoke every session that person holds, and its follow up list names the
admin reset endpoint and the staff row blanking erasure path as this feature's to own. The session
machinery to do all of that exists and is tested; what is missing is the code path that calls it.

**Every staff row is referenced by the floor.** Spec 0003's schema points `visits`, `rounds`,
`order_lines`, `bills`, and `audit_log` at `staff` through composite foreign keys. A person who
leaves cannot be deleted without either breaking that history or rewriting it, so the leaving path
has to be a state on the row, not a removal of it.

**Compliance scope**: this feature is the second place the platform writes personal data about a
real person (spec 0006 was the first), and the first place one person writes another person's
credentials. GDPR style rules apply to a staff member's name, email address, and password hash.
Audit logging is therefore not optional here: these actions are access control changes.

The cost of not deciding is that features 12 and 13 get built and demonstrated with one account,
and the first restaurant to try the product on a real service discovers there is no way to put a
waiter on the floor.

## Options considered

### Option 1: Admin managed accounts on the existing staff table, with a forced password change

The admin creates the account, types or generates a starting password, and hands it over. The row
carries a flag saying the password is not the person's own yet, and that person can do nothing but
choose a new one until they do. Everything else (role change, password reset, deactivation) is an
admin action on the same row.

**Pros**:
- No new dependency of any kind. No email provider, no crate, no web library, no environment
  variable, no infrastructure change.
- The forced change means the window in which an admin knows a waiter's working password is exactly
  one sign in long, which is the only real weakness of a handed over credential.
- Every rule spec 0006 wrote for this feature is implemented as written, not reinterpreted.
- It matches how a restaurant actually onboards someone: the manager is standing next to them.

**Cons**:
- The starting password crosses a human channel. Said aloud in a kitchen, it can be overheard, and
  an admin who picks `waiter123` has picked it for a real account until that person signs in.
- It puts one more screen in front of a new waiter on what is probably a busy first shift.
- The admin sees the person's email address and can reset their password at will, so an admin
  account is worth more than it was yesterday.

### Option 2: Emailed invite links

The admin enters a name, an address, and a role; the platform sends a single use link; the person
sets a password nobody else ever knows.

**Pros**:
- The strongest credential story available. No password ever exists that two people know.
- The same token machinery answers feature 24 (password recovery), so the cost is paid once.
- Onboarding works when the person is not standing in the restaurant.

**Cons**:
- Takes on an email provider (AWS SES on this stack), which means a domain to verify, a sending
  reputation to keep, bounce handling, and a production dependency that can fail silently. Spec 0006
  weighed exactly this and declined.
- Needs a token table, an expiry policy, a resend path, and a screen that works for a signed out
  visitor holding a token, all before a waiter can take a single order.
- A kitchen porter may not have a work email address at all, which is common in this trade and turns
  the strongest option into a blocker.

### Option 3: A restaurant join code that staff redeem themselves

The restaurant gets a code; a new person signs up with it, lands with no role or a default role, and
the admin approves and assigns the role afterwards.

**Pros**:
- The admin types nothing per person, which scales best for a restaurant hiring in a group.
- The person chooses their own password from the start.

**Cons**:
- It creates a public, unauthenticated write path into a tenant, which is precisely what the scope
  row forbids ("staff never register themselves") and what spec 0006's tenant model was built to
  make impossible.
- A leaked code is an open door until somebody rotates it, and the rotation, the expiry, and the
  approval queue are three mechanisms to design for a restaurant with six staff.
- It inverts the audit story: the row exists before an admin ever approved it.

### Option 4: Shared accounts per role

One waiter login and one chef login for the whole restaurant.

**Pros**:
- Nothing to build at all beyond two extra rows in the seed. Zero onboarding friction.

**Cons**:
- Destroys attribution. `visits.opened_by_staff_id` and every audit row stop naming a person, which
  makes feature 18 (sales reports per waiter) impossible and the audit log decorative.
- One departure means changing a password everyone uses, so in practice nobody ever changes it.
- It is not recoverable later: turning one shared account into six real ones means rewriting
  history that was never recorded.

## Rationale

Option 1 is chosen because the two forces that actually constrain this decision both point at it.
There is no email provider and spec 0006 decided there would not be one yet, which rules out
Option 2 on a dependency the project has already refused once; and the scope row's own words, staff
never register themselves, rule out Option 3 on a tenant boundary the whole data model is built to
hold. Option 4 is the cheapest thing that could work and is rejected on the grounds that it would
quietly delete the attribution features 17 and 18 depend on, which is the kind of choice that cannot
be walked back later.

That leaves the real work in containing Option 1's one genuine weakness, the handed over password.
The forced change is what contains it: the flag is set whenever an admin writes somebody's password,
and it is cleared only when that person writes their own, so a password two people know is never
valid for longer than one sign in. Blocking it at the API rather than only in the browser is what
makes that a control instead of a suggestion, and it costs one boolean on a function that already
returns four other things about the session.

The advisory lock on the last admin count was weighed against locking the active admin rows
themselves with `SELECT ... FOR UPDATE`, which would serialise the same two guard rails without a
second locking primitive. It was not chosen because the advisory lock keys on the restaurant rather
than on a row set, so it covers a deactivation and a demotion racing each other identically, and
because spec 0006 already established the primitive for the sign in throttle, so this is one pattern
used twice rather than two patterns used once.

The engineer chose not to make the email address editable, which is the right call for a field that
is the sign in identifier and is unique across the whole platform; the consequence, that a mistyped
address stays claimed forever, is recorded honestly in the Consequences rather than smoothed over,
and the follow up says what would fix it.

Two smaller choices follow the project rather than the feature. The version column and the
conditional update come straight from the rule in `api/AGENTS.md` and from spec 0008, which just
established that shape for menu rows; staff would otherwise be the one editable table that breaks
it. And the advisory lock on the last admin count is spec 0006's own pattern for the sign in
throttle, reused because the failure it prevents, a restaurant with no admin and no way back in, is
the worst outcome this feature can produce.

## Evidence: what spec 0006 handed forward

From spec 0006's `## Follow-up` and its state transitions table, the items this feature was named as
the owner of, and what became of each:

| Item spec 0006 recorded | Resolved here |
|---|---|
| Deactivating an account revokes every session of that person | Built. AC-10 |
| Changing a role revokes every session of that person | Built. AC-8 |
| Resetting somebody's password revokes every session of that person | Built. AC-9 |
| The admin resets a password endpoint | Built. `POST /api/staff/{id}/password`, AC-9 |
| The staff row blanking erasure path | **Not built.** The engineer chose deactivate and reactivate only. Carried forward as a follow up, still unowned |
| `staff.last_sign_in_at`, added for feature 10's staff list | Used. AC-6 |
| An owner who is the only admin and forgets their password is locked out | **Partly closed.** A second admin can now reset the first one's password (AC-9). A restaurant with exactly one admin is still stuck, and still waits on feature 24 |
