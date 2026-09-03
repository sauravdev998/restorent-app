# 0006. Accounts, restaurants, and roles: reasoning

The build spec is [index.md](index.md). This file holds the decision record: the problem, the
options weighed, and why one won. `/develop` does not read it.

## Context

> ⚠️ Premise note: this feature builds authentication by hand, which is one of the most reliable
> ways a small team ships a breach. The usual failure is not the password check, it is everything
> around it: session fixation, a token that cannot be revoked, a cookie missing an attribute, a
> reset link that never expires, a role check somebody forgot on one endpoint. Spec 0001 already
> weighed this and chose to own it, so the framing is not being reopened here. What this spec does
> instead is remove the surface that usually sinks a hand written auth system: there is no
> federation, no single sign on, no multi factor, no OAuth, no refresh token rotation, and no
> password reset email, because none of those was chosen. What is left is issue a random token,
> store its hash, look it up on every request, and gate on a role, which is a small amount of code
> against a schema spec 0003 already shaped for exactly this. The role check is put in the type
> system rather than in a convention, for the same reason tenant scoping was. The residual risk is
> real and is named in the Consequences.

The platform has a database, a design system, and a translation layer, and no way for anybody to
sign in. Every screen so far runs against a development only placeholder: the API reads an
`x-restaurant-id` header and refuses every scoped request outside development, and the browser
reads a fixed restaurant id out of the address bar. Nothing above this feature can ship until that
placeholder is replaced, because the whole product is per restaurant data and there is currently no
honest way to know which restaurant a request belongs to.

The forces that shape the answer are mostly already fixed. Spec 0001 chose to own authentication
rather than buy it, and named the machinery: opaque random session tokens stored hashed, delivered
in an httpOnly cookie, with `argon2id` for passwords. Spec 0003 then built the schema for it: a
`staff` table with a platform wide unique email, a `sessions` table, and two `SECURITY DEFINER`
functions owned by a third database role, `auth_lookup`, which are the only two paths in the entire
system that read across restaurants. That pair exists solely so this feature can look somebody up
before it knows which restaurant they belong to. Spec 0005 then wrote two placeholder modules
shaped precisely to consume this feature's sign in response, and left three items owed.

The product context matters as much as the technical one. This is a staff tool inside a restaurant,
not a consumer product. The people signing in are an owner, a handful of waiters, and a couple of
chefs, on a shared phone or a screen bolted to a kitchen wall, twenty times a shift, often with wet
hands. The owner is physically present, which changes what account recovery has to be. There is no
email provider in the stack and no budget line for one. There is also no payment provider and no
public self signup traffic yet, so the account that matters is the owner's, and it is created once.

Two constraints bound the decision. First, feature 10 requires a deactivated account to be refused
at once, which rules out any credential the server cannot revoke. Second, this feature creates the
platform's first personal data: a person's name, their email address, and a password hash. That
puts it in GDPR style scope for erasure and for what may appear in a log line. Spec 0003 committed
to two erasure paths at the schema level and neither is built.

Not deciding is not an option: nineteen features sit above this one and every single one of them
needs to know who is asking.

## Options considered

### Option 1: An opaque session token in a cookie, resolved against the database on every request

Sign in checks the password with `argon2id`, generates 32 random bytes, stores their SHA-256 in
`sessions.token_hash`, and returns the raw value in an httpOnly, `SameSite=Lax` cookie. Every
request resolves that hash through `resolve_session`, which yields the staff member, the role, and
the restaurant that scopes the rest of the transaction.

**Pros**:

- Revocation is instant and total, because the server holds the truth. Deactivating an account,
  changing a role, or resetting a password takes effect on the next request with no window.
- It is exactly what spec 0003's schema, its two lookup functions, and its `auth_lookup` role were
  built for. Nothing has to be added to the tenancy story.
- The token is meaningless if stolen from a log or a backup: it is a hash, not a credential.
- Small enough to read in one sitting, which is what makes a hand written auth layer reviewable.

**Cons**:

- One indexed database read on every single request, including every heartbeat of every open live
  stream.
- The correctness of the cookie's attributes is now the project's problem, and getting one wrong is
  quiet rather than loud.

### Option 2: A hosted identity provider

Hand identity to a product built for it: an external service holds the accounts, runs the sign in
screens, and issues tokens the API verifies. Roles and the restaurant id ride along as claims.

**Pros**:

- The parts that are genuinely hard and genuinely dangerous (credential storage, brute force
  defence, account recovery, and later multi factor or single sign on) become somebody else's
  operational problem, staffed by people who do only that.
- It comes with an email delivery path for recovery, which this stack does not have.

**Cons**:

- It contradicts spec 0001's whole basis: everything is owned so no vendor can change pricing,
  deprecate a feature, or hold the user table. The user table is the one table you least want held.
- Per user pricing on a product whose users are waiters, in a business selling to restaurants at a
  low price point, scales the wrong way.
- Revocation becomes eventual, bounded by token lifetime, unless every request calls the provider,
  which is the same per request cost as option 1 plus a network hop and a third party outage.
- The `staff` and `sessions` tables and both lookup functions spec 0003 built would be dead weight.

### Option 3: Stateless signed tokens

Issue a short lived signed token carrying the staff id, restaurant id, and role, verified in
process with no database read, refreshed by a longer lived refresh token.

**Pros**:

- No database read per request, which is the cheapest possible authentication at scale.
- Horizontally trivial: any instance can verify without shared state.

**Cons**:

- Revocation is the known weak point, and this product has a hard revocation requirement from
  feature 10. The usual answer is a deny list checked on every request, which is the database read
  the option existed to avoid, plus a second mechanism to keep correct.
- Refresh token rotation, replay detection, and clock skew are three more things to get right, in a
  feature whose premise note is that hand written auth fails on exactly this kind of surface area.
- A stale role travels inside the token, so a demoted admin keeps admin powers until it expires.

### Option 4: An existing Rust session library

Use a maintained crate that already implements sessions for this framework (the `axum-login` and
`tower-sessions` family), with a Postgres store, rather than writing the session layer.

**Pros**:

- Battle tested session handling, cookie attributes, and store abstraction, for a fraction of the
  code, and a real community finding the bugs.
- Straightforwardly the right answer if the session layer were the interesting part of this feature.

**Cons**:

- Its store wants to own its own table and its own connection, which collides with the two things
  this codebase holds most tightly: every query goes through a scoped transaction, and the only two
  unscoped read paths are named `SECURITY DEFINER` functions owned by a separate role. Fitting a
  generic store into that means either bending the library or bending the tenancy rule.
- It would sit beside, not replace, the schema spec 0003 already designed, so the project would
  carry two session models.
- The saving is smaller than it looks: what the library gives is roughly the token issue and cookie
  handling, which is the easy half. The role gate, the tenant scoping, the revocation triggers, and
  the throttle are all still ours.

## Rationale

Option 1 wins because the two constraints from Context point at it directly and the alternatives
each break one of them. Feature 10's requirement that a deactivated account is refused at once is a
revocation requirement, and only a server held session satisfies it without bolting a deny list
onto a design that exists to avoid one, which is option 3's fatal shape. Spec 0001's decision to own
the platform rules out option 2 on cost and on principle, and option 2 would also make revocation
eventual, losing the same property. Option 4 is the one that would have been chosen in a greenfield
codebase, and it loses only because spec 0003 already built this exact schema, including a database
role whose entire purpose is to own the two functions this feature calls. Adopting a library now
would mean carrying two session models and arguing with the tenancy rule that the whole project
rests on.

The per request database read that option 1 costs was accepted in spec 0001 and is accepted again
here with eyes open. It is an index hit on a unique `bytea` column, on a connection the request is
about to use anyway, and it is the price of the one property the design was chosen for. Caching it
would trade away exactly that property, which is why no cache is used, not even on read only paths.

Three product answers shaped the rest. Recovery is an admin resetting a password rather than an
email link, because the admin is standing in the same building and because an email provider is
infrastructure this project does not have and would have to operate. Sessions slide over fourteen
days under a ninety day ceiling, because sign in is an email address and a password typed on a phone
keyboard, and a short session would push people toward a password taped to the till. And nothing
locks permanently on failed attempts, because a whole restaurant shares one internet address and a
lockout is a way to take a restaurant offline during service.

The role gate goes into the type system rather than into a convention for the same reason spec 0001
put tenant scoping there: a written rule is forgotten by feature nineteen, a type is not. An admin
only handler names `Actor<Admin>` in its signature, so a missing gate is a visible absence in a
function signature rather than a missing line in the middle of a body.

Two decisions deliberately do less than they could. The admin resets somebody else's password
endpoint moves to feature 10, where staff creation, role changes, and deactivation live together and
have a screen to sit on, because building it here means writing rules about who may act on whom in a
product that contains exactly one account. And neither erasure path is built, because no screen can
reach one; what this spec does instead is write down that the schema supports both, who owns each,
and that an erasure request today is a manual database operation.
