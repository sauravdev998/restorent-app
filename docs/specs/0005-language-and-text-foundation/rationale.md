# 0005. Language and text foundation: reasoning

The build spec is [index.md](index.md). This file is the decision record: the problem, the options,
and why one was chosen. `/develop` does not read it.

## Context

> ⚠️ Premise note: this feature depends on a decision that has no spec. Feature 7 (accounts,
> restaurants, and roles) owns sign in and sessions, and it is the thing that would tell a screen
> who is reading it and which restaurant they belong to. Building language resolution before that
> exists means the resolution rule is real and tested while its inputs come from a development only
> placeholder, so a whole class of bug (wrong settings for a real restaurant) cannot surface until
> feature 7 lands. The framing is still right, because the alternative is twenty screens with
> English baked into them, but it is an assumption and not a fact: this spec assumes feature 7 will
> return the restaurant's settings and the signed in person's language in its sign in response, and
> will provide a write path scoped to the caller's own row. Those shapes are written into the API
> surface table so feature 7 inherits them rather than rediscovering them, and design feature 7
> before building anything that leans harder on them.

The platform is sold to many restaurants, and the people using it are not one audience. An owner
sits at a desk with a laptop, a waiter works a phone through a service, and a chef reads a screen
across a hot kitchen. Nothing says those three read the same language, and nothing says a
restaurant in Delhi and a restaurant in Manchester want the same number and date format. The scope
puts this decision in the foundations for one reason: text is the cheapest thing in the project to
get right now and among the most expensive to retrofit. Twenty features are queued behind it, each
one adding screens.

Some of the ground is already laid, and it constrains what follows. Feature 1 wired `i18next`,
`react-i18next`, and the browser language detector, initialised them before the first render, and
left a comment saying feature 6 owns the real decisions: how a person's choice is stored, and how
files are loaded on demand. Sixteen components already call `t()` against a single
`web/src/locales/en/common.json` of roughly ninety keys, so the "no literal in a component" habit
exists but nothing enforces it. Feature 5 committed the platform to WCAG 2.2 level AA, backed by
three automatic gates (a contrast script, an axe pass, and lint rules), and swept the codebase onto
logical properties with a lint rule that rejects any physical direction utility, quoting `dir="rtl"`
in its own error message. Feature 5 also chose the Geist type family, which has no Devanagari
glyphs at all.

Spec [0003](../0003-core-data-model/index.md) gives every restaurant a `currency_code`, a
`currency_decimals`, and a `timezone`, and fixes that the local day a bill belongs to comes from
the restaurant's timezone and never the server's. It gives no table anywhere a language column.
Money crosses the wire as an exact decimal and every stored figure is already rounded to that
bill's own currency decimals, so whatever formats money for display must not undo that.

Two things about the surrounding order matter. Feature 7 (accounts, restaurants, and roles) comes
after this one, so there is no session, no sign in, and no authenticated write path yet:
`RestaurantScope` on the API and `currentRestaurantId()` on the web are both development only
placeholders that refuse to work anywhere else. And features 16 (printing) and 19 (the public
marketing page) are slices away, but both will need an answer to "which language is this in" that
neither can derive on its own, because paper has no reader to ask and a public page has no
restaurant.

The cost of not deciding is concrete and compounding: every screen shipped before this lands is a
screen someone unpicks later, in a codebase where twenty features are still to come.

## Options considered

### Option 1: keep the single file, add a second language, decide the rest later

Add `web/src/locales/hi/common.json` beside the English one, bundle both, translate what exists,
and leave formatting, storage of the preference, and enforcement to the features that hit them.

**Pros**:

- Almost no work. It is a file and a switcher, and the second language ships this week.
- Nothing existing has to move, so the sixteen tested components stay untouched.

**Cons**:

- The single file is roughly ninety keys today and several hundred by slice 5, and every surface
  downloads all of it, including a waiter's phone downloading the admin reporting strings.
- Nothing enforces the rule, so the twenty first component with English baked in is found by a
  person, if at all.
- It leaves the load bearing questions open exactly where they are expensive: what language a
  printed bill is in, whether an English reading owner in India gets Indian number grouping, and
  which timezone a displayed time uses. Each one then gets answered ad hoc, differently, inside
  whichever feature trips over it first.

### Option 2: keep i18next, split by surface, and pull formatting away from language (chosen)

Namespaces mirroring the three surfaces, loaded on demand per language and per namespace; a shared
catalogue file both Rust and TypeScript read; three additive database columns with a fixed
resolution rule; formatting driven by a restaurant level locale that is deliberately not the
interface language; and three automatic gates in the same shape feature 5 already established.

**Pros**:

- Every load bearing question is answered once and in one place, including the two that belong to
  features that do not exist yet.
- The enforcement matches the pattern already in the codebase, so the rule holds without anyone
  remembering it.
- Formatting being independent of language is the only arrangement where a bill formats the same
  for every member of staff, and where an English reading owner still gets their own region's
  numbers.
- Adding a language stays a file operation, which is what the scope's success condition actually
  asks for.

**Cons**:

- It is the most work of the three, and it moves ninety keys and sixteen tested files as part of it.
- It leans on a placeholder module for restaurant settings until feature 7 lands, so part of the
  formatting path is not exercised against real data yet.
- Two settings that sound alike (a language and a formatting locale) now exist side by side, and
  the difference has to be explained to every future contributor.

### Option 3: translate on the server

Move the wording to the API: the request carries a language, the API returns translated prose in
its error bodies and preformatted money and date strings alongside the raw values, and the web app
mostly renders what it is handed.

**Pros**:

- One implementation of every rule, shared by the web app, a printed bill, and any future client,
  so a receipt and a screen can never disagree.
- It answers the printing question for free, since the server is where a printed document would be
  rendered anyway.

**Cons**:

- It builds a second translation system, in Rust, in a second file format, for the same strings,
  and that system has to be kept in step with the first one by hand.
- The API stops being a data API. A caller that wants to add two figures now has to know which
  field is the number and which is the picture of the number.
- It needs a language on every request before feature 7 exists to put one there, which is the same
  ordering problem, moved somewhere it is harder to place a placeholder.
- It puts formatting on the server where the browser already ships correct, per locale formatting
  in every language the catalogue could ever grow to.

### Option 4: an external translation management platform

Keep the keys in a hosted service, sync them into the build, and let non engineers edit copy in a
web interface.

**Pros**:

- Translators work without touching the repository, which genuinely matters once there are several
  languages and a rolling copy review.
- Missing key reporting and translation memory come as part of it.

**Cons**:

- A third party service, an account, an API token, and a build step that can fail, added to a
  platform with two languages and one engineer.
- Copy stops living in git, so it is no longer reviewed, blamed, or reverted with the code that
  uses it.
- It solves a coordination problem this project does not have yet, and the migration into one
  later is easy precisely because the keys are plain JSON.

## Rationale

Option 2 is chosen because the forces in Context are almost all about the shape of things that
already exist, not about translation as a technology. `i18next` is installed, initialised, and
already in sixteen components, so the technology question is settled and the real question is what
gets decided around it. Feature 5's three automatic gates set the house pattern for how a rule is
kept, and a lint error plus a build failure plus a fake language is that same pattern applied to
text. Feature 5's logical property sweep and its lint rule already point at `dir="rtl"`, so
carrying a `direction` per language and setting the `dir` attribute is finishing work that is half
done rather than starting new work.

The separation of formatting locale from interface language is the least obvious call here and the
one worth defending. It looks like an extra setting nobody asked for. The alternative, deriving
number and date format from the language a person reads, means an English reading owner in India
sees their own revenue in American grouping with month first dates, and means the same bill
formats two ways depending on which member of staff prints it. Spec 0003 already treats currency,
decimals, and timezone as properties of the restaurant rather than of a reader, and put the bill's
own currency snapshot on the bill so a bill printed a year later still says what was charged. A
formatting locale on the restaurant is the same idea applied to the last piece, and pinning digits
to Latin follows from the same place: a bill is read by customers and accountants, not only by
whoever chose the setting.

Option 3 was the closest competitor and was rejected on the API's own conventions. `api/AGENTS.md`
is emphatic that nothing crosses the wire as anything but a data DTO and that one error shape
carries a stable machine readable code precisely so a client can branch on it. That code already
exists, so the client mapping it to a translated sentence is using a seam that was built for this,
while server side prose would add a parallel system for no gain that the printing feature cannot
get later by other means. The one thing Option 3 genuinely wins, printed documents, is answered
here by a rule rather than by architecture: paper uses the restaurant's default language, which is
knowable without a request language.

The decision to ship no endpoint deserves a word, because it looks like an unfinished Tracer
Bullet. The thread this feature has to prove is language resolution and formatting, and that thread
does pierce every layer: a real column, real Rust validation, a real resolver, a real switcher,
real Devanagari on screen. What it cannot prove is an authenticated write, because authentication
is the next feature. Writing that endpoint against a scope extractor that refuses every request
outside development would produce something untestable that feature 7 rewrites, so the preference
persists in browser storage now and gains a server side home the moment there is a session to
attach it to, with nothing on the client changing when it does.

Hindi shipping machine translated is a deliberate, recorded compromise. The purpose of a second
language in a foundation feature is to prove the machinery under a script that is not Latin, with
plural rules and line boxes that are not English's. Machine translation proves all of that. It does
not prove the wording is good, which is why the folder is marked unreviewed and a native speaker
pass is a tracked follow up rather than an assumption.

The engineer chose "always English on the sign in screen" over detecting the browser's language.
That is a deliberate trade of first visit convenience for predictability, and it is why
`i18next-browser-languagedetector` is removed rather than reconfigured: a dependency whose entire
purpose is the behaviour being rejected is better deleted than switched off. Remembering an
explicit choice on the device recovers most of the convenience, since a house phone behind the bar
is set once and stays set.
