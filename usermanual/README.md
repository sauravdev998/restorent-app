# User manual

The client facing user manual, written in [Typst](https://typst.app).

- `main.typ`: the manual itself
- `images/`: screenshots taken from the running app
- `user-manual.pdf`: the built manual, ready to send
- `scripts/`: what took the screenshots

## Build the PDF

```bash
cd usermanual
typst compile main.typ user-manual.pdf
```

Use `typst watch main.typ user-manual.pdf` while editing. The manual uses the Noto Sans font.

## Retake the screenshots

The screenshots come from a throwaway database with sample data ("Spice Garden",
Anita, Ravi and Meera), never from the shared dev database, so they stay clean.

```bash
# 1. Fresh Postgres on port 5436, migrated, seeded, renamed, and the API started against it
bash usermanual/scripts/fresh-database.sh

# 2. The web dev server, in another terminal
pnpm dev:web

# 3. Take the screenshots (admin first, then the service flow)
cd web
node ../usermanual/scripts/shots-admin.mjs
node ../usermanual/scripts/shots-service.mjs

# 4. Trim the white space around the three sign in screens
cd ../usermanual/images
for f in 01-sign-in 02-register 05-choose-password; do
  magick $f.png -crop +0+160 +repage -trim +repage -bordercolor white -border 30 $f.png
done

# 5. Stop the throwaway database and the API
docker rm -f manual-db; pkill -x api
```

The service script backdates two kitchen tickets in the database, so the kitchen
screenshot shows the amber and red warnings.
