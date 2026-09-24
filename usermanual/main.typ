// Restaurant Operations: user manual.
// Build with `typst compile main.typ` from this folder (see README.md).

#let brand = rgb("#0369a1")
#let ink = rgb("#0f172a")
#let muted = rgb("#475569")
#let line-col = rgb("#cbd5e1")
#let soft = rgb("#f1f5f9")

#let version = "1.0"
#let issued = "September 2026"

#set document(title: "Restaurant Operations: User Manual", author: "Restaurant Operations")
#set text(font: "Noto Sans", size: 10.5pt, fill: ink, lang: "en")
#set par(justify: false, leading: 0.7em, spacing: 1.1em)
#set page(
  paper: "a4",
  margin: (x: 2.2cm, top: 2.4cm, bottom: 2.2cm),
  header: context {
    if here().page() > 2 {
      set text(size: 8.5pt, fill: muted)
      grid(columns: (1fr, auto), [Restaurant Operations · User Manual], [Version #version])
      v(-0.4em)
      line(length: 100%, stroke: 0.5pt + line-col)
    }
  },
  footer: context {
    if here().page() > 1 {
      set text(size: 8.5pt, fill: muted)
      align(center, counter(page).display("1"))
    }
  },
)

#set heading(numbering: "1.1")
#show heading.where(level: 1): it => {
  pagebreak(weak: true)
  v(0.4em)
  block(below: 1.2em)[
    #text(size: 11pt, fill: brand, weight: "bold")[
      #if it.numbering != none [Chapter #counter(heading).display("1")]
    ]
    #v(-0.3em)
    #text(size: 22pt, weight: "bold")[#it.body]
    #v(-0.4em)
    #line(length: 3cm, stroke: 2pt + brand)
  ]
}
#show heading.where(level: 2): it => block(above: 1.6em, below: 0.8em, text(size: 14pt, weight: "bold", it))
#show heading.where(level: 3): it => block(above: 1.3em, below: 0.6em, text(size: 11.5pt, weight: "bold", fill: brand, it.body))

#show figure.caption: it => text(size: 9pt, fill: muted, it)
#set figure(gap: 0.8em)
#show figure: set block(breakable: false)

// ---- helpers -------------------------------------------------------------

// A button, link, or label exactly as it reads on screen.
#let ui(label) = box(
  fill: soft,
  stroke: 0.5pt + line-col,
  inset: (x: 4pt, y: 1.5pt),
  outset: (y: 2pt),
  radius: 3pt,
  text(size: 9.5pt, weight: "medium", label),
)

// A screenshot with a thin frame.
#let shot(path, caption, width: 100%) = figure(
  box(stroke: 0.6pt + line-col, radius: 4pt, clip: true, image("images/" + path, width: width)),
  caption: caption,
)

// Two or three phone screenshots side by side, each with its own caption.
#let phones(..items) = {
  let list = items.pos()
  figure(
    grid(
      columns: list.len() * (1fr,),
      column-gutter: 14pt,
      ..list.map(((path, cap)) => stack(
        spacing: 6pt,
        box(stroke: 0.6pt + line-col, radius: 6pt, clip: true, image("images/" + path, width: 100%)),
        align(center, text(size: 8.5pt, fill: muted, cap)),
      ))
    ),
    caption: none,
    kind: image,
    supplement: none,
  )
}

#let callout(title, color, body) = block(
  width: 100%,
  fill: color.lighten(90%),
  stroke: (left: 3pt + color),
  inset: (x: 12pt, y: 10pt),
  radius: (right: 4pt),
  [#text(weight: "bold", fill: color.darken(20%), title) \ #body],
)
#let tip(body) = callout("Tip", rgb("#15803d"), body)
#let note(body) = callout("Good to know", brand, body)
#let caution(body) = callout("Take care", rgb("#b45309"), body)

// ---- cover ----------------------------------------------------------------

#page(header: none, footer: none, margin: 0pt)[
  #block(width: 100%, height: 58%, fill: brand, inset: (x: 2.4cm, y: 2.6cm))[
    #set text(fill: white)
    #text(size: 12pt, weight: "medium", tracking: 1.5pt)[USER MANUAL]
    #v(1.2em)
    #text(size: 34pt, weight: "bold")[Restaurant \ Operations]
    #v(0.8em)
    #text(size: 14pt)[Orders at the table, tickets live in the kitchen, \ and a bill at the end of the meal.]
    #v(1fr)
    #text(size: 10.5pt)[For restaurant owners, managers, waiters and chefs]
  ]
  #block(inset: (x: 2.4cm, y: 1.4cm))[
    #grid(
      columns: (auto, 1fr),
      column-gutter: 18pt,
      row-gutter: 10pt,
      text(fill: muted)[Version], [#version],
      text(fill: muted)[Issued], [#issued],
      text(fill: muted)[Covers], [Admin, Waiter and Kitchen screens],
      text(fill: muted)[Works on], [Any modern web browser: phone, tablet, laptop or kitchen screen],
    )
    #v(1.5em)
    #text(size: 9pt, fill: muted)[
      The screenshots in this manual come from a sample restaurant, "Spice Garden", with
      sample staff and dishes. Your own screens show your restaurant's name, menu, tables and people.
    ]
  ]
]

// ---- contents -------------------------------------------------------------

#page(header: none)[
  #text(size: 22pt, weight: "bold")[Contents]
  #v(1em)
  #show outline.entry.where(level: 1): it => {
    v(0.6em)
    strong(it)
  }
  #outline(title: none, indent: 1.2em, depth: 2)
]

// ===========================================================================

= Welcome

Restaurant Operations runs the service in your restaurant, from the moment a party sits down
to the moment the bill is closed. Waiters take orders at the table on a phone or tablet. The
kitchen sees each order appear on its screen within a second or two. Chefs mark each dish done
as it leaves the pass, and the waiter is alerted when food is ready to collect.

There is nothing to install. Everybody opens the app in a web browser and signs in with the
email address and password their admin gave them.

== Three roles, three screens

Every person in your restaurant has one role, and the role decides which screen they see.

#table(
  columns: (auto, 1fr, auto),
  inset: 8pt,
  stroke: (x, y) => if y == 0 { (bottom: 1pt + ink) } else { (bottom: 0.5pt + line-col) },
  table.header([*Role*], [*What they do*], [*Best device*]),
  [*Admin*], [Sets up the restaurant: the menu, the tables, the staff accounts and the restaurant settings.], [Laptop or tablet],
  [*Waiter*], [Opens tables, takes orders, sends them to the kitchen, serves food and closes the bill.], [Phone],
  [*Chef*], [Watches incoming tickets on the kitchen screen and marks dishes done.], [Tablet or wall screen in the kitchen],
)

A waiter cannot open the admin or kitchen screens, and a chef cannot open the admin or waiter
screens. Each restaurant sees only its own data.

== How an order moves

#let stage(n, who, what) = box(
  width: 100%,
  stroke: 0.6pt + line-col,
  radius: 4pt,
  inset: 8pt,
  fill: white,
)[
  #text(size: 8pt, fill: brand, weight: "bold")[#n · #upper(who)] \
  #text(size: 9.5pt, what)
]
#let arrow = align(center + horizon, text(fill: brand, size: 14pt)[→])

#block(breakable: false)[
  #grid(
    columns: (1fr, 12pt, 1fr, 12pt, 1fr, 12pt, 1fr, 12pt, 1fr),
    align: horizon,
    stage("1", "Waiter", [Opens the table and adds dishes]),
    arrow,
    stage("2", "Waiter", [Sends the round to the kitchen]),
    arrow,
    stage("3", "Chef", [Cooks and taps each dish *Done*]),
    arrow,
    stage("4", "Waiter", [Gets the ready alert and serves]),
    arrow,
    stage("5", "Waiter", [Closes the bill; the table is free]),
  )
]

A table stays open for the whole meal. The waiter can send as many rounds as the party orders,
such as starters first and mains later. Each round goes to the kitchen as its own ticket, and
every round ends up on the same bill.

== Before you start

- A device with a recent web browser (Chrome, Edge, Safari or Firefox) and an internet connection.
- The web address of your Restaurant Operations app, given to you when your restaurant was set up.
- For staff: the email address and first password your admin gave you.
- For the kitchen: a screen the chefs can read from a few steps away. A landscape tablet or a
  wall-mounted screen works well.

// ===========================================================================

= Getting started

== Registering your restaurant

The owner does this once, when the restaurant first joins. Whoever registers becomes the
restaurant's first *admin*.

#grid(
  columns: (1fr, 1fr),
  column-gutter: 20pt,
  [
    + Open the app and choose #ui[Register a restaurant] under the sign in form.
    + Enter the *restaurant name*.
    + Choose your *country*. It sets your currency, time zone and how numbers and dates are
      written. For India that is rupees (₹) and Indian time.
    + Enter *your name*, *email address* and a *password* of at least 10 characters.
    + Choose #ui[Register]. You are signed in straight away as the admin.
  ],
  shot("02-register.png", [Registering a new restaurant.]),
)

#note[Staff never register themselves. Once the restaurant exists, the admin creates an account
for every waiter and chef (see @staff).]

== Signing in

#grid(
  columns: (1fr, 1fr),
  column-gutter: 20pt,
  [
    + Open the app's web address.
    + Enter your *email address* and *password*.
    + Choose #ui[Sign in].

    You land on your own screen: admins on the admin screens, waiters on the floor plan, and
    chefs on the kitchen pass.

    After several wrong passwords the app pauses sign in attempts for a few minutes, to keep
    accounts safe. Wait, then try again.
  ],
  shot("01-sign-in.png", [The sign in screen.]),
)

== Your first sign in: choose your own password

When an admin creates your account, they give you a *first password*. It works once. The first
time you sign in with it, the app asks you to choose your own.

#grid(
  columns: (1fr, 1fr),
  column-gutter: 20pt,
  [
    + Type the password you were given.
    + Type a new password of at least 10 characters.
    + Choose #ui[Save and carry on].

    From then on you sign in with your new password. Nobody else, including the admin, can see it.
  ],
  shot("05-choose-password.png", [Choosing your own password at first sign in.]),
)

== Finding your way around

The bar across the top of every screen holds:

- the *screens you can use*: for an admin these are #ui[Menu], #ui[Tables], #ui[Staff] and
  #ui[Settings]; waiters see #ui[Waiter] and chefs see #ui[Kitchen];
- #ui[Account], for your own details;
- the *connection light*. #ui[Connected] means live updates are arriving. If it changes, see
  @trouble;
- the *language* menu. The app is available in *English* and *Hindi* (हिन्दी).

== Your account

#ui[Account] is where every person, whatever their role, manages their own details.

#shot("19-account.png", [The Account screen.], width: 88%)

- *Your name*: how your name appears on tickets and to your colleagues.
- *Your language*: the language you read the app in, on every device you sign in on. Leave it on
  "Whatever the restaurant uses" to follow the restaurant's language.
- *Password*: enter your current password and a new one. Changing it signs you out on every
  other device, and keeps you signed in on this one.
- *Sign out*: signs out this device only.

#tip[On a shared kitchen tablet, sign in once as the chef and leave it signed in. On a personal
phone, sign out at the end of your shift if other people use the phone.]

// ===========================================================================

= Admin guide

This chapter is for the owner or manager who sets the restaurant up. Everything you change
here reaches every waiter's and chef's screen within a few seconds. There is no "publish"
button.

== Menu

Open #ui[Menu]. The menu is made of *categories* (such as Starters and Mains), each holding
*dishes*. Waiters see the menu in exactly this order, so arrange it the way your printed menu reads.

#shot("10-admin-menu.png", [The admin Menu screen. Each dish shows its diet mark, price and availability switch.])

=== Adding a category

+ Choose #ui[Add a category].
+ Type a name of up to 60 characters, such as "Desserts" or "Breads".
+ Choose #ui[Add category]. It is added at the end of the menu.

Waiters see a category once it has at least one dish in it.

=== Adding a dish

#grid(
  columns: (1fr, 1.25fr),
  column-gutter: 20pt,
  [
    + Choose #ui[Add a dish], either at the top of the screen or inside a category.
    + Choose the *category*.
    + Type the *name* exactly as waiters and the kitchen should read it (up to 80 characters).
    + Add an optional *description*: what a waiter reads out to a guest.
    + Enter the *price* in your currency.
    + Choose the *diet* mark: Vegetarian, Non vegetarian or Contains egg.
    + Choose #ui[Add dish].
  ],
  shot("11-admin-add-dish.png", [Adding a dish.]),
)

The new dish is added at the end of its category, marked available.

=== Changing a dish

Choose #ui[Edit] next to the dish to change its name, description, price, diet mark or
category. Dishes already sent to the kitchen keep the name and price they were ordered at, so a
price change never alters an open bill.

=== Switching a dish off for the night

When the kitchen runs out of something, turn off the dish's switch. It changes from
#ui[Available] to #ui[Off tonight]. Within seconds every waiter sees the dish greyed out and
can no longer order it. Turn the switch on again when the dish is back.

Chefs can do the same from the kitchen screen (see @kitchen-menu).

#note[Switching a dish off never touches dishes already ordered. If a waiter has the dish in
an unsent basket, the app marks it and asks them to take it out before sending.]

=== Putting the menu in order

Drag a dish or a category by its handle (the six dots on the left) to move it. With a
keyboard, focus the handle and press Space to pick it up, the arrow keys to move it, and Space
again to drop it. Escape cancels the move.

=== Removing and restoring

Choose #ui[Remove] on a dish or category. Removed items move to the *Archived* list at the
bottom of the screen and disappear from every waiter's screen. Bills that already charged for a
removed dish still read exactly as they did. Choose #ui[Put back] in Archived to restore one.

A category has to be empty before it can be removed. Move or remove its dishes first.

== Tables <tables>

Open #ui[Tables]. Here you define the tables your waiters open, grouped into optional
*sections* such as "Main room", "Terrace" or "Bar".

#shot("13-admin-floor.png", [The Tables screen, with one section holding four tables.])

=== Adding tables

#grid(
  columns: (1fr, 1.2fr),
  column-gutter: 20pt,
  [
    + Choose #ui[Add tables].
    + Choose #ui[One table] or #ui[Several tables].
    + For one table, give it a short *label* (up to 12 characters, such as "12" or "Bar 3") and,
      if you like, the number of *seats*.
    + For several tables, give a numbered range, such as 5 to 12. The app lists any labels
      already in use.
    + Choose the *section*, then add.
  ],
  shot("14-admin-add-table.png", [Adding a table.]),
)

Use #ui[Add a section] to create a new area. Choose #ui[Rename] or #ui[Edit] to change a
section or table, and drag the handles to set the order waiters see the floor in.

=== Removing a table

Choose #ui[Remove] next to the table. A table with a party at it cannot be removed until the
waiter closes it. Removed tables and sections go to *Archived*, where #ui[Put back] restores them.

== Staff <staff>

Open #ui[Staff]. It lists everybody who works at the restaurant, their role and when they last
signed in.

#shot("15-admin-staff.png", [The Staff screen.])

=== Adding a person

#grid(
  columns: (1fr, 1.25fr),
  column-gutter: 20pt,
  [
    + Choose #ui[Add somebody].
    + Enter their *name* as it should read on a ticket.
    + Enter their *email address*. They sign in with it, and it cannot be changed later, so
      check it carefully.
    + Type a *first password* of at least 10 characters, or choose #ui[Suggest a strong one].
    + Choose their *role*: Admin, Waiter or Chef.
    + Choose #ui[Create the account].
  ],
  shot("16-admin-add-staff.png", [Adding a new chef.]),
)

The app then shows the email address and first password one last time. Hand them over in
person. At their first sign in, the new person is asked to choose their own password.

#shot("17-admin-staff-created.png", [The details to hand over. The password is not shown again.], width: 80%)

Until they choose their own password, the Staff list shows "Has not chosen a password yet"
under their name.

=== Changing someone's account

Each row on the Staff list has these actions:

#table(
  columns: (auto, 1fr),
  inset: 7pt,
  stroke: (x, y) => (bottom: 0.5pt + line-col),
  [#ui[Rename]], [Change how their name appears.],
  [#ui[Role]], [Change what they may do, for example from Waiter to Admin. They are signed out and land on their new screen when they sign in again.],
  [#ui[Reset password]], [Give them a new first password when they have forgotten theirs. They choose their own again at next sign in.],
  [#ui[Switch off]], [For somebody who has left. They are signed out at once and cannot sign in. Their name stays on every order and bill they handled. Switched off accounts appear under *Switched off*, and can be brought back.],
)

#caution[The restaurant always needs at least one admin who can sign in, so the app will not let
you switch off or demote the last one. You also cannot switch off or change the role of your own
account; ask another admin.]

== Restaurant settings

Open #ui[Settings].

#grid(
  columns: (1fr, 1.1fr),
  column-gutter: 20pt,
  [
    - *Restaurant name* and *address*. The address will be printed on bills.
    - *Time zone*: every time on every screen is shown in it, for example Asia/Kolkata.
    - *Restaurant language*: what the kitchen screen reads, and what staff who have not chosen
      a language of their own see.
    - *Number and date format*: how money, numbers and dates are written, for example
      ₹1,00,000.00 in the Indian format.
    - *When the kitchen screen warns*: after how many minutes a waiting ticket turns *amber*
      (getting late), and after how many it turns *red* (late). The defaults are 10 and 15
      minutes. Set them to suit your kitchen.

    Choose #ui[Save settings] when you are done.
  ],
  shot("18-admin-settings.png", [Restaurant settings.]),
)

#note[Your currency is fixed by the country you registered in. To change it, contact support.]

// ===========================================================================

= Waiter guide

This chapter is for waiters. The waiter screens are designed for a phone held in one hand.
There are two views, switched at the top: #ui[Floor] and #ui[Orders].

== The floor

#grid(
  columns: (1fr, 1fr),
  column-gutter: 22pt,
  [
    The *Floor* shows every table, grouped by section, with its number of seats.

    - A *free* table shows #ui[Open table].
    - An *occupied* table shows when it was opened and which waiter has it, with #ui[View] to
      open it.

    Every waiter sees the same floor, and it updates live. When a colleague opens a table,
    it turns occupied on your phone too. A table can never hold two open bills at once.

    Turn on #ui[Mine] to see only your own tables.
  ],
  phones(("20-waiter-floor.png", [The floor with four free tables.])),
)

== Taking an order

=== 1. Open the table

Tap #ui[Open table] on the table the party sat at. The table is now yours, and its screen
opens with the menu.

=== 2. Add dishes to the basket

Tap #ui[+] next to a dish to add one to the basket. Tap again to add more. Dishes that are off
tonight are greyed out and struck through, and cannot be added.

In the basket you can:

- use #ui[−] and #ui[+] to change the quantity;
- write a *note* for the kitchen on any line, such as "less spicy, no onions" (up to 140
  characters). The note reaches the kitchen ticket exactly as written;
- tap #ui[One separately] to put one portion on its own line, for example when only one of two
  naans should be made without butter.

#phones(
  ("21-waiter-table-empty.png", [The table's menu.]),
  ("22-waiter-basket.png", [The basket, with a note for the kitchen.]),
  ("23-waiter-sent.png", [After sending: round 1 is cooking.]),
)

=== 3. Send it to the kitchen

Tap #ui[Send 4 dishes] (the button counts the dishes for you). The basket becomes *Round 1*
under *Sent to the kitchen*, and the ticket appears on the kitchen screen within a second or two.

When the party orders more, add dishes again and send them. They become Round 2, then Round 3,
each its own kitchen ticket, all on the same bill.

#tip[The basket is kept on your phone until you send it. If you step away or the page reloads,
the dishes you added are still there.]

== Following the kitchen

Each round and each dish shows its status:

#table(
  columns: (auto, 1fr),
  inset: 7pt,
  stroke: (x, y) => (bottom: 0.5pt + line-col),
  [*Cooking*], [The kitchen has it and is working on it.],
  [*Ready*], [The chef has marked it done. It is waiting on the pass for you to collect.],
  [*Served*], [You have taken it to the table.],
  [*Cancelled*], [It was taken off the order. It is not charged.],
)

The clock beside each round shows how long ago it was sent.

== When food is ready

#grid(
  columns: (1fr, 1fr),
  column-gutter: 22pt,
  [
    When the chef finishes a round, the waiter who owns the table gets:

    - a *sound*, and a vibration on phones that support it;
    - a *green alert* at the top of the screen, naming the table and the dishes;
    - a *ready badge* on the table in the Orders view.

    Tap #ui[Acknowledge] to dismiss the alert. If you don't, it reminds you again after two minutes.

    Then collect the food and serve it:

    - tap #ui[Serve] next to a dish to mark that one dish served, or
    - tap #ui[Serve all ready] to mark everything ready on that round served in one go.
  ],
  phones(("24-waiter-ready-alert.png", [The ready alert over the Orders view.])),
)

#tip[The first time you use the app on a new phone, tap anywhere on the screen once so the
browser allows the alert sound.]

== The Orders view

Tap #ui[Orders] for a list of every open table and its rounds, with tables that have food ready
sorted to the top. Serve straight from this list, or tap a table to open it. With #ui[Mine]
turned on, the list shows only your tables.

#phones(
  ("25-waiter-orders.png", [Orders, with a round ready.]),
  ("26-waiter-served.png", [Everything served, ready to close.]),
  ("27-waiter-bill-closed.png", [The closed bill with its total.]),
)

== Taking over a colleague's table

If a colleague goes on a break or ends their shift, open their table and tap #ui[Take over].
The table becomes yours, and its ready alerts now come to you.

== Cancelling a dish

#grid(
  columns: (1fr, 1fr),
  column-gutter: 22pt,
  [
    If a guest changes their mind, or a dish was entered by mistake:

    + Tap #ui[Cancel] under the dish in *Sent to the kitchen*.
    + Choose *why*: Guest changed their mind, Entered by mistake, Kitchen can't make it, or Other.
    + Add details if you like. Details are required when you choose Other.
    + Tap #ui[Cancel the dish].

    The dish comes off the bill at once, and the kitchen sees it as *Cancelled* on its ticket so
    it stops cooking. Tap #ui[Keep the dish] to back out.

    A dish that has already been served cannot be cancelled.
  ],
  phones(("28-waiter-cancel-dish.png", [Cancelling a dish, with a reason.])),
)

== Moving a party to another table

#grid(
  columns: (1fr, 1fr),
  column-gutter: 22pt,
  [
    Tap #ui[Move table] on the table's screen, choose a free table, and tap #ui[Move]. The party
    takes its rounds, its bill and anything in the basket with it, and the kitchen sees the new
    table number.
  ],
  phones(("29-waiter-move-table.png", [Moving a party to a free table.])),
)

== Closing the bill

The *Bill so far* card at the bottom of the table screen keeps a running subtotal as rounds are
sent. When the party is finished:

+ Make sure every dish is marked *Served*. The app won't close a bill while food is still on
  its way.
+ Tap #ui[Close the bill].

The bill gets its number and total, and the table turns free on everybody's floor, ready for
the next party.

If everything on a table was cancelled, or nothing was ordered at all, closing it makes no bill;
it simply frees the table.

// ===========================================================================

= Kitchen guide <kitchen>

This chapter is for chefs. The kitchen screen is built to be read from a few steps away and used
with wet or gloved hands: big text, big buttons, and nothing to scroll through on a normal night.
It has two tabs: #ui[The pass] and #ui[Menu].

== Setting up the kitchen screen

+ Sign in as a chef on the kitchen tablet or screen.
+ *Tap the screen once.* Until you do, a "Sound is off" notice is shown, because browsers only
  allow sound after someone has touched the page. After that tap, a chime sounds for every new
  ticket.
+ Leave it open. The screen stays awake on its own while the pass is showing.

#shot("30-kitchen-empty.png", [The pass on a quiet moment, before the screen has been tapped for sound.], width: 80%)

== Reading the pass

New tickets appear on their own, with no refresh, *oldest first*. Each ticket is one round from
one table and shows:

- the *table* and the *round* number;
- *Waiting*: how long ago it was sent;
- every dish with its quantity, and the waiter's *note* underneath, exactly as written.

#shot("31-kitchen-pass.png", [Three tickets on the pass. Table 1 has waited too long (red), table 2 is getting late (amber), and table 3 has just arrived.])

The waiting time changes colour as a ticket ages:

#table(
  columns: (auto, 1fr),
  inset: 7pt,
  stroke: (x, y) => (bottom: 0.5pt + line-col),
  [*Plain*], [On time.],
  [#text(fill: rgb("#b45309"), weight: "bold")[Amber], with a flame], [Getting late. By default after 10 minutes.],
  [#text(fill: rgb("#b91c1c"), weight: "bold")[Red], with a warning sign], [Late. By default after 15 minutes.],
)

The admin can change both times in Settings.

== Marking food done

- Tap #ui[Done] next to a dish when it leaves the pass. Only that dish is marked.
- Tap #ui[All done] on a ticket to mark every dish on it at once.

When the last dish on a ticket is done, the ticket turns *Ready* by itself and moves down to
*Ready to collect*, and the waiter is alerted. It leaves the pass when the waiter serves it.

#shot("33-kitchen-ready.png", [Table 2's ticket is ready and waiting for the waiter. Tables 1 and 3 are still cooking.], width: 88%)

=== Made a mistake?

Tap #ui[Back on] next to a dish in *Ready to collect* to put it back on the stove. The ticket
returns to *Cooking* until the dish is done again.

=== Ran out of something?

Tap #ui[Ran out] next to a dish on a ticket to take it off that order. The waiter sees it as
cancelled, and it comes off the bill. Also switch the dish off in the #ui[Menu] tab so nobody
orders it again tonight.

=== Cancelled dishes

When a waiter cancels a dish, the ticket shows it in a *Cancelled* strip with the reason, so
you know to stop cooking it.

== Switching dishes off from the kitchen <kitchen-menu>

#shot("34-kitchen-menu.png", [The kitchen Menu tab. Fish pakora is off tonight.], width: 88%)

Open the #ui[Menu] tab and turn a dish's switch *Off* the moment you run out. Every waiter sees
it greyed out within seconds. Turn it *On* again when it is back.

// ===========================================================================

= Troubleshooting <trouble>

#let qa(q, a) = block(breakable: false, below: 1.1em)[
  #text(weight: "bold")[#q] \
  #a
]

#qa[The screen says "Live updates have stopped" or "This screen is not live".][
  The device lost its connection to the server for a moment. Everything you tap still works and
  nothing is lost. Reload the page when you can and it catches up. If it keeps happening, check
  the device's Wi-Fi.
]

#qa[I forgot my password.][
  Ask your admin to choose #ui[Reset password] on the Staff screen. They give you a new first
  password, and you choose your own at your next sign in.
]

#qa[It says "Too many attempts".][
  After several wrong passwords the app pauses sign in for a few minutes. Wait, then try again.
]

#qa[It says "Somebody else changed this first".][
  Another person saved a change to the same thing a moment before you. The screen now shows the
  latest version; look it over and make your change again if it is still needed.
]

#qa[I can't close the bill.][
  Every dish has to be marked *Served* first. Look for anything still Cooking or Ready and serve
  it, or cancel it if it is not going to the table.
]

#qa[A dish is greyed out and I can't add it.][
  It has been switched off for the night by the kitchen or the admin. Tell the guest it is not
  available, or ask the kitchen whether it is coming back.
]

#qa[The kitchen screen makes no sound.][
  Tap the screen once. Browsers only play sound after the page has been touched. Also check that
  the device's volume is up.
]

#qa[A new staff member can't sign in.][
  Check the email address on the Staff screen. It cannot be changed, so if it was typed wrong,
  switch that account off and create a new one. If their account is under *Switched off*, bring
  it back.
]

#qa[Someone sees the wrong screens after a role change.][
  They need to sign in again. A role change signs them out on purpose.
]

// ===========================================================================

= Quick reference

#let card(title, color, body) = block(
  width: 100%,
  stroke: 0.8pt + color,
  radius: 5pt,
  inset: 0pt,
  breakable: false,
  below: 14pt,
)[
  #block(width: 100%, fill: color, inset: (x: 10pt, y: 7pt), radius: (top: 5pt), text(fill: white, weight: "bold", title))
  #block(inset: (x: 10pt, y: 8pt), body)
]

#card("Waiter: one table, start to finish", rgb("#0369a1"))[
  + #ui[Floor] → #ui[Open table]
  + #ui[+] to add dishes; add a note if needed
  + #ui[Send] → the kitchen gets Round 1
  + More orders later? Add and #ui[Send] again → Round 2, 3…
  + Ready alert → #ui[Acknowledge] → collect → #ui[Serve] / #ui[Serve all ready]
  + Everything served → #ui[Close the bill]
]

#card("Chef: the pass", rgb("#b45309"))[
  + Tap the screen once at the start of service, to turn the sound on
  + Oldest ticket first; watch for amber and red times
  + #ui[Done] per dish, or #ui[All done] per ticket
  + Wrong tap? #ui[Back on]. Out of an ingredient? #ui[Ran out], and switch the dish off in #ui[Menu]
]

#card("Admin: setting up a new restaurant", rgb("#15803d"))[
  + #ui[Settings]: check the name, time zone, language and kitchen warning times
  + #ui[Tables]: add sections and tables
  + #ui[Menu]: add categories, then dishes with prices and diet marks
  + #ui[Staff]: add each waiter and chef, and hand over their first password
  + Ask one waiter and one chef to sign in and send a test order together
]
