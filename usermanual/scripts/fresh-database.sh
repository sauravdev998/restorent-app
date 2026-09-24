set -e
cd "$(dirname "$0")/../.."
S=${TMPDIR:-/tmp}
pkill -x api || true
docker rm -f manual-db >/dev/null 2>&1 || true
docker run -d --rm --name manual-db -p 5436:5432 -e POSTGRES_USER=restaurant_owner -e POSTGRES_PASSWORD=local_dev_only -e POSTGRES_DB=restaurant -v $PWD/api/scripts/init-roles.sql:/docker-entrypoint-initdb.d/init-roles.sql:ro postgres:17-alpine >/dev/null
until docker exec manual-db pg_isready -U restaurant_owner -d restaurant -h 127.0.0.1 >/dev/null 2>&1; do sleep 1; done; sleep 2
export DATABASE_URL=postgres://app_api:local_dev_only@localhost:5436/restaurant OWNER_DATABASE_URL=postgres://restaurant_owner:local_dev_only@localhost:5436/restaurant RUST_LOG=warn
sqlx migrate run --source api/migrations --database-url "$OWNER_DATABASE_URL" >/dev/null
cargo run -q --manifest-path api/Cargo.toml --bin seed >/dev/null
docker exec -i manual-db psql -U restaurant_owner -d restaurant -tA >/dev/null <<'SQL'
update restaurants set name='Spice Garden';
update staff set display_name='Anita Sharma', email='anita@spicegarden.test' where email='admin@example.test';
update staff set display_name='Ravi Kumar', email='ravi@spicegarden.test' where email='waiter@example.test';
update staff set display_name='Meera Iyer', email='meera@spicegarden.test' where email='chef@example.test';
SQL
nohup cargo run -q --manifest-path api/Cargo.toml > $S/api.log 2>&1 &
for i in $(seq 1 60); do curl -sf http://127.0.0.1:8080/api/health >/dev/null && break; sleep 1; done
echo reset done
