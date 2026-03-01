#!/bin/bash
# Compile kwwhat dbt models for DuckDB and generate kwwhat_models.js
#
# Usage:
#   ./tools/kwwhat/compile_kwwhat.sh [path-to-kwwhat-repo]
#
# Defaults to ../kwwhat relative to the project root.
# Requires Docker.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
KWWHAT_DIR="${1:-$(cd "$PROJECT_ROOT/.." && pwd)/kwwhat}"
ANALYTICS_DIR="$PROJECT_ROOT/assets/analytics"
MODELS_JS="$PROJECT_ROOT/assets/js/kwwhat_models.js"

if [ ! -d "$KWWHAT_DIR/models" ]; then
    echo "ERROR: kwwhat repo not found at $KWWHAT_DIR"
    echo "Usage: $0 [path-to-kwwhat-repo]"
    exit 1
fi

echo "==> kwwhat repo: $KWWHAT_DIR"

echo "==> Building dbt Docker image..."
docker build -t kwwhat-dbt -f "$SCRIPT_DIR/Dockerfile.dbt" "$SCRIPT_DIR"

echo "==> Installing dbt packages..."
docker run --rm \
    -v "$KWWHAT_DIR:/kwwhat" \
    -v "$SCRIPT_DIR/dbt/profiles.yml:/root/.dbt/profiles.yml" \
    kwwhat-dbt deps

echo "==> Compiling kwwhat models for DuckDB..."
docker run --rm \
    -v "$KWWHAT_DIR:/kwwhat" \
    -v "$SCRIPT_DIR/dbt/profiles.yml:/root/.dbt/profiles.yml" \
    kwwhat-dbt compile

COMPILED_DIR="$KWWHAT_DIR/target/compiled/kwwhat/models"
if [ ! -d "$COMPILED_DIR" ]; then
    echo "ERROR: Compiled models not found at $COMPILED_DIR"
    exit 1
fi

echo ""
echo "==> Checking for unresolved Jinja..."
if grep -r '{{' "$COMPILED_DIR" 2>/dev/null || \
   grep -r '{%' "$COMPILED_DIR" 2>/dev/null; then
    echo "WARNING: Found unresolved Jinja templates in compiled SQL!"
    exit 1
else
    echo "OK: No unresolved Jinja found."
fi

echo ""
echo "==> Copying compiled SQL to $ANALYTICS_DIR..."
rm -rf "$ANALYTICS_DIR"
mkdir -p "$ANALYTICS_DIR/staging"
mkdir -p "$ANALYTICS_DIR/intermediate/outages"
mkdir -p "$ANALYTICS_DIR/marts"

cp "$COMPILED_DIR/staging/stg_ocpp_logs.sql"                    "$ANALYTICS_DIR/staging/"
cp "$COMPILED_DIR/staging/stg_ports.sql"                         "$ANALYTICS_DIR/staging/"
cp "$COMPILED_DIR/intermediate/int_status_changes.sql"           "$ANALYTICS_DIR/intermediate/"
cp "$COMPILED_DIR/intermediate/int_transactions.sql"             "$ANALYTICS_DIR/intermediate/"
cp "$COMPILED_DIR/intermediate/int_connector_preparing.sql"      "$ANALYTICS_DIR/intermediate/"
cp "$COMPILED_DIR/intermediate/int_meter_values.sql"             "$ANALYTICS_DIR/intermediate/"
cp "$COMPILED_DIR/intermediate/outages/int_faulted_outages.sql"  "$ANALYTICS_DIR/intermediate/outages/"
cp "$COMPILED_DIR/intermediate/outages/int_offline_outages.sql"  "$ANALYTICS_DIR/intermediate/outages/"
cp "$COMPILED_DIR/marts/dim_dates.sql"                           "$ANALYTICS_DIR/marts/"
cp "$COMPILED_DIR/marts/fact_charge_attempts.sql"                "$ANALYTICS_DIR/marts/"
cp "$COMPILED_DIR/marts/fact_interval_data.sql"                  "$ANALYTICS_DIR/marts/"
cp "$COMPILED_DIR/marts/fact_downtime_daily.sql"                 "$ANALYTICS_DIR/marts/"
cp "$COMPILED_DIR/marts/fact_visits.sql"                         "$ANALYTICS_DIR/marts/"

echo "==> Copied $(find "$ANALYTICS_DIR" -name '*.sql' | wc -l | tr -d ' ') SQL files"

echo ""
echo "==> Patching SQL for DuckDB-WASM compatibility..."
node -e "
var fs = require('fs');
var path = require('path');
var dir = process.argv[1];

function patchSql(sql) {
  // 1) json_extract_path_text -> json_extract_string with JSONPath
  sql = sql.replace(
    /json_extract_path_text\(([^,]+),\s*'([^']+)'\s*\)/g,
    function(_, expr, p) {
      var jp = p.charAt(0) === '[' ? '\$' + p : '\$.' + p;
      return 'json_extract_string(' + expr + \", '\" + jp + \"')\";
    }
  );
  // 2) ::jsonb -> ::JSON
  sql = sql.split('::jsonb').join('::JSON');
  // 3) cardinality() -> len() for LISTs
  sql = sql.split('cardinality(').join('len(');
  // 4) unnest(json_extract(x, '\$')) as alias -> unnest(x::JSON[]) as alias(value)
  sql = sql.replace(
    /unnest\(json_extract\((\w+),\s*'\\$'\)\)\s*\n\s*as\s+(\w+)/g,
    function(_, expr, alias) {
      return 'unnest(' + expr + '::JSON[]) as ' + alias + '(value)';
    }
  );
  // 5) Adapt the incremental processing window to the actual data range.
  //    kwwhat uses a hardcoded start_processing_date '2025-10-01' meant for its seed data.
  //    Replace it with a dynamic lookup of the earliest timestamp in the actual data,
  //    and keep the 3-month window which is enough for a game session.
  sql = sql.split(\"cast( '2025-10-01' as timestamp)\").join(
    '(select coalesce(min(ingested_timestamp), TIMESTAMP \\'2025-10-01\\') from \"memory\".\"main\".\"stg_ocpp_logs\")'
  );
  // 6) DuckDB 1.0+ uses 1-based array indexing. kwwhat's SQL uses [0] which returns NULL.
  sql = sql.split('connector_ids[0]').join('connector_ids[1]');
  sql = sql.split('transaction_ids[0]').join('transaction_ids[1]');
  // 7) Cap uncapped +3 month incremental boundaries to the actual data range.
  //    Some models use (from_timestamp + 3 months) as to_timestamp without
  //    least(...), letting unclosed periods extend months beyond the data.
  sql = sql.replace(
    /\(from_timestamp \+ cast\(3 as bigint\) \* interval 1 month\) as to_timestamp/g,
    'least(\\n        (from_timestamp + cast(3 as bigint) * interval 1 month),\\n        (select max(ingested_timestamp) from \"memory\".\"main\".\"stg_ocpp_logs\")\\n    ) as to_timestamp'
  );
  return sql;
}

function walk(d) {
  fs.readdirSync(d).forEach(function(f) {
    var full = path.join(d, f);
    if (fs.statSync(full).isDirectory()) { walk(full); return; }
    if (!full.endsWith('.sql')) return;
    var original = fs.readFileSync(full, 'utf8');
    var patched = patchSql(original);
    if (patched !== original) {
      fs.writeFileSync(full, patched);
      console.log('  patched: ' + path.relative(dir, full));
    }
  });
}
walk(dir);
console.log('  done');
" "$ANALYTICS_DIR"

echo ""
echo "==> Validating patched SQL against seed data in DuckDB..."
docker run --rm \
    -v "$KWWHAT_DIR:/kwwhat" \
    -v "$ANALYTICS_DIR:/analytics" \
    --entrypoint python \
    kwwhat-dbt -c "
import duckdb, os, sys

MODEL_ORDER = [
    ('stg_ocpp_logs', 'staging/stg_ocpp_logs.sql'),
    ('stg_ports', 'staging/stg_ports.sql'),
    ('int_status_changes', 'intermediate/int_status_changes.sql'),
    ('int_transactions', 'intermediate/int_transactions.sql'),
    ('int_connector_preparing', 'intermediate/int_connector_preparing.sql'),
    ('int_meter_values', 'intermediate/int_meter_values.sql'),
    ('int_faulted_outages', 'intermediate/outages/int_faulted_outages.sql'),
    ('int_offline_outages', 'intermediate/outages/int_offline_outages.sql'),
    ('dim_dates', 'marts/dim_dates.sql'),
    ('fact_charge_attempts', 'marts/fact_charge_attempts.sql'),
    ('fact_interval_data', 'marts/fact_interval_data.sql'),
    ('fact_downtime_daily', 'marts/fact_downtime_daily.sql'),
    ('fact_visits', 'marts/fact_visits.sql'),
]

con = duckdb.connect(':memory:')

# Load seed data
con.execute(\"\"\"
    CREATE TABLE ocpp_raw AS
    SELECT timestamp, id, action,
           replace(msg, chr(39), chr(34)) as msg
    FROM read_csv('/kwwhat/seeds/ocpp_1_6_synthetic_logs_14d.csv',
                  auto_detect=true, header=true)
\"\"\")
con.execute(\"\"\"
    CREATE TABLE ports_raw AS
    SELECT * FROM read_csv('/kwwhat/seeds/ports.csv',
                           auto_detect=true, header=true)
\"\"\")

# Create schema matching compiled SQL source references
con.execute(\"ATTACH ':memory:' AS \\\"RAW\\\"\")
con.execute('CREATE SCHEMA \"RAW\".\"SEED\"')
con.execute('CREATE TABLE \"RAW\".\"SEED\".\"ocpp_1_6_synthetic_logs_14d\" AS SELECT * FROM ocpp_raw')
con.execute('CREATE TABLE \"RAW\".\"SEED\".\"ports\" AS SELECT * FROM ports_raw')

ocpp_count = con.execute('SELECT count(*) FROM ocpp_raw').fetchone()[0]
ports_count = con.execute('SELECT count(*) FROM ports_raw').fetchone()[0]
print(f'  seed data: {ocpp_count} ocpp rows, {ports_count} ports')

failed = False
for name, path in MODEL_ORDER:
    sql_path = f'/analytics/{path}'
    with open(sql_path) as f:
        sql = f.read()
    try:
        con.execute(f'CREATE TABLE \"{name}\" AS ({sql})')
        count = con.execute(f'SELECT count(*) FROM \"{name}\"').fetchone()[0]
        print(f'  {name}: {count} rows')
    except Exception as e:
        print(f'  {name}: FAILED - {e}')
        failed = True
        break

if failed:
    print('VALIDATION FAILED')
    sys.exit(1)

# Quick sanity: check key tables have rows
for t in ['fact_charge_attempts', 'fact_visits', 'int_status_changes']:
    n = con.execute(f'SELECT count(*) FROM \"{t}\"').fetchone()[0]
    if n == 0:
        print(f'  WARNING: {t} has 0 rows')

print('  OK: all 13 models validated')
"

echo ""
echo "==> Generating $MODELS_JS..."

# Model execution order (topological)
MODEL_ORDER=(
    "stg_ocpp_logs:staging/stg_ocpp_logs.sql"
    "stg_ports:staging/stg_ports.sql"
    "int_status_changes:intermediate/int_status_changes.sql"
    "int_transactions:intermediate/int_transactions.sql"
    "int_connector_preparing:intermediate/int_connector_preparing.sql"
    "int_meter_values:intermediate/int_meter_values.sql"
    "int_faulted_outages:intermediate/outages/int_faulted_outages.sql"
    "int_offline_outages:intermediate/outages/int_offline_outages.sql"
    "dim_dates:marts/dim_dates.sql"
    "fact_charge_attempts:marts/fact_charge_attempts.sql"
    "fact_interval_data:marts/fact_interval_data.sql"
    "fact_downtime_daily:marts/fact_downtime_daily.sql"
    "fact_visits:marts/fact_visits.sql"
)

# Generate JS file with embedded SQL
cat > "$MODELS_JS" << 'HEADER'
// Auto-generated by tools/compile_kwwhat.sh -- do not edit manually.
// Contains compiled kwwhat dbt models for DuckDB execution.
window.__kwtycoon_kwwhat_models = [
HEADER

FIRST=true
for entry in "${MODEL_ORDER[@]}"; do
    NAME="${entry%%:*}"
    PATH_SUFFIX="${entry#*:}"
    SQL_FILE="$ANALYTICS_DIR/$PATH_SUFFIX"

    if [ ! -f "$SQL_FILE" ]; then
        echo "ERROR: Missing compiled SQL: $SQL_FILE"
        exit 1
    fi

    if [ "$FIRST" = true ]; then
        FIRST=false
    else
        echo "," >> "$MODELS_JS"
    fi

    # Escape the SQL for embedding in a JS string (backslashes, backticks, ${})
    ESCAPED_SQL=$(sed -e 's/\\/\\\\/g' -e 's/`/\\`/g' -e 's/\${/\\${/g' "$SQL_FILE")

    printf '  { name: "%s", sql: `%s` }' "$NAME" "$ESCAPED_SQL" >> "$MODELS_JS"
done

cat >> "$MODELS_JS" << 'FOOTER'

];
FOOTER

echo "==> Generated kwwhat_models.js ($(wc -c < "$MODELS_JS" | tr -d ' ') bytes)"
echo ""
echo "==> Done! Files ready:"
echo "    $ANALYTICS_DIR/ (13 SQL files)"
echo "    $MODELS_JS"
