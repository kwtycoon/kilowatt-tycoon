#!/bin/bash
# Test exported OCPP CSVs against kwwhat pipeline in DuckDB.
#
# Usage:
#   ./tools/kwwhat/test_export.sh <ocpp_messages.csv> [ocpp_ports.csv]
#
# Runs the patched SQL models from assets/analytics/ against the
# exported game data and reports row counts + key metrics.
# Requires Docker (uses the kwwhat-dbt image).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ANALYTICS_DIR="$PROJECT_ROOT/assets/analytics"

OCPP_CSV="${1:?Usage: $0 <ocpp_messages.csv> [ocpp_ports.csv]}"
PORTS_CSV="${2:-}"

if [ ! -f "$OCPP_CSV" ]; then
    echo "ERROR: File not found: $OCPP_CSV"
    exit 1
fi

if [ ! -d "$ANALYTICS_DIR" ]; then
    echo "ERROR: Patched SQL not found at $ANALYTICS_DIR"
    echo "Run tools/kwwhat/compile_kwwhat.sh first"
    exit 1
fi

# Ensure Docker image exists
docker inspect kwwhat-dbt > /dev/null 2>&1 || {
    echo "==> Building dbt Docker image..."
    docker build -t kwwhat-dbt -f "$SCRIPT_DIR/Dockerfile.dbt" "$SCRIPT_DIR"
}

echo "==> Testing exported data against kwwhat pipeline"
echo "    OCPP CSV: $OCPP_CSV ($(wc -l < "$OCPP_CSV" | tr -d ' ') lines)"
if [ -n "$PORTS_CSV" ] && [ -f "$PORTS_CSV" ]; then
    echo "    Ports CSV: $PORTS_CSV ($(wc -l < "$PORTS_CSV" | tr -d ' ') lines)"
fi
echo ""

docker run --rm \
    -v "$ANALYTICS_DIR:/analytics:ro" \
    -v "$(cd "$(dirname "$OCPP_CSV")" && pwd):/data:ro" \
    ${PORTS_CSV:+-v "$(cd "$(dirname "$PORTS_CSV")" && pwd):/ports_data:ro"} \
    --entrypoint python \
    kwwhat-dbt -c "
import duckdb, sys, json

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

# Load exported OCPP messages
ocpp_file = '/data/$(basename "$OCPP_CSV")'
con.execute(f\"\"\"
    CREATE TABLE ocpp_raw AS
    SELECT * FROM read_csv('{ocpp_file}', auto_detect=true, header=true)
\"\"\")
ocpp_count = con.execute('SELECT count(*) FROM ocpp_raw').fetchone()[0]
print(f'=== Source Data ===')
print(f'  ocpp_messages: {ocpp_count} rows')

# Show action breakdown
actions = con.execute('''
    SELECT action, count(*) as n
    FROM ocpp_raw
    WHERE action != ''
    GROUP BY action ORDER BY n DESC
''').fetchall()
for a, n in actions:
    print(f'    {a}: {n}')

# Show CallResult count
cr_count = con.execute(\"\"\"SELECT count(*) FROM ocpp_raw WHERE action = ''\"\"\").fetchone()[0]
print(f'    CallResult (empty action): {cr_count}')

# Load ports
ports_csv = '${PORTS_CSV:+/ports_data/$(basename "${PORTS_CSV:-}")}'
if ports_csv:
    con.execute(f\"\"\"
        CREATE TABLE ports_raw AS
        SELECT * FROM read_csv('{ports_csv}', auto_detect=true, header=true)
    \"\"\")
else:
    # Create empty ports table with expected schema
    con.execute(\"\"\"
        CREATE TABLE ports_raw (
            charge_point_id VARCHAR, location_id VARCHAR, port_id VARCHAR,
            connector_id VARCHAR, connector_type VARCHAR,
            commissioned_ts VARCHAR, decommissioned_ts VARCHAR
        )
    \"\"\")
ports_count = con.execute('SELECT count(*) FROM ports_raw').fetchone()[0]
print(f'  ports: {ports_count} rows')

# Sample a few messages to check format
print()
print('=== Message Samples ===')
samples = con.execute('''
    SELECT action, msg FROM ocpp_raw
    WHERE action IN ('StartTransaction', 'StatusNotification', 'Heartbeat')
    LIMIT 3
''').fetchall()
for action, msg in samples:
    preview = msg[:120] + '...' if len(msg) > 120 else msg
    print(f'  {action}: {preview}')

# Check if msg column has valid JSON
print()
bad_json = con.execute(\"\"\"
    SELECT count(*) FROM ocpp_raw
    WHERE msg IS NOT NULL AND msg != ''
    AND json_valid(msg) = false
\"\"\").fetchone()[0]
if bad_json > 0:
    print(f'  WARNING: {bad_json} rows have invalid JSON in msg column')
    sample = con.execute(\"\"\"
        SELECT action, left(msg, 80) FROM ocpp_raw
        WHERE json_valid(msg) = false LIMIT 3
    \"\"\").fetchall()
    for a, m in sample:
        print(f'    {a}: {m}')
else:
    print('  JSON validation: all rows OK')

# Create source schema
con.execute(\"ATTACH ':memory:' AS \\\"RAW\\\"\")
con.execute('CREATE SCHEMA \"RAW\".\"SEED\"')
con.execute('CREATE TABLE \"RAW\".\"SEED\".\"ocpp_1_6_synthetic_logs_14d\" AS SELECT * FROM ocpp_raw')
con.execute('CREATE TABLE \"RAW\".\"SEED\".\"ports\" AS SELECT * FROM ports_raw')

# Run models
print()
print('=== Model Execution ===')
failed = False
for name, path in MODEL_ORDER:
    sql_path = f'/analytics/{path}'
    with open(sql_path) as f:
        sql = f.read()
    try:
        con.execute(f'CREATE TABLE \"{name}\" AS ({sql})')
        count = con.execute(f'SELECT count(*) FROM \"{name}\"').fetchone()[0]
        status = 'OK' if count > 0 else 'EMPTY'
        print(f'  {name}: {count} rows [{status}]')
    except Exception as e:
        print(f'  {name}: FAILED - {e}')
        failed = True
        break

if failed:
    print()
    print('PIPELINE FAILED')
    sys.exit(1)

# Report metrics
print()
print('=== Metrics ===')
try:
    att = con.execute('SELECT count(*) as total, count(*) FILTER (WHERE is_successful) as ok FROM fact_charge_attempts').fetchone()
    rate = f'{att[1]/att[0]*100:.1f}%' if att[0] > 0 else 'N/A'
    print(f'  Charge Attempt Success: {rate} ({att[1]}/{att[0]})')

    vis = con.execute('SELECT count(*) as total, count(*) FILTER (WHERE is_successful) as ok FROM fact_visits').fetchone()
    rate = f'{vis[1]/vis[0]*100:.1f}%' if vis[0] > 0 else 'N/A'
    print(f'  Visit Success: {rate} ({vis[1]}/{vis[0]})')

    dt = con.execute('SELECT type, round(sum(duration_minutes),1), count(*) FROM fact_downtime_daily GROUP BY type').fetchall()
    for t, mins, n in dt:
        print(f'  {t} Downtime: {mins} min ({n} incidents)')

    tot = con.execute('''
        SELECT
            (SELECT count(*) FROM int_transactions) as txns,
            (SELECT count(*) FROM int_status_changes) as sc,
            (SELECT count(*) FROM int_connector_preparing) as cp
    ''').fetchone()
    print(f'  Transactions: {tot[0]}')
    print(f'  Status Changes: {tot[1]}')
    print(f'  Connector Preparing: {tot[2]}')
except Exception as e:
    print(f'  Error querying metrics: {e}')

# Diagnose empty results
if att[0] == 0:
    print()
    print('=== Diagnosis: 0 charge attempts ===')
    # Check if StatusNotification has Preparing status
    prep = con.execute(\"\"\"
        SELECT count(*) FROM int_status_changes WHERE status = 'Preparing'
    \"\"\").fetchone()[0]
    print(f'  StatusNotification with Preparing: {prep}')

    # Check if StartTransaction messages exist
    st = con.execute(\"\"\"
        SELECT count(*) FROM stg_ocpp_logs WHERE action = 'StartTransaction'
    \"\"\").fetchone()[0]
    print(f'  StartTransaction in stg_ocpp_logs: {st}')

    # Check message_type_id extraction
    mt = con.execute(\"\"\"
        SELECT message_type_id, count(*) as n FROM stg_ocpp_logs
        GROUP BY message_type_id ORDER BY n DESC
    \"\"\").fetchall()
    print(f'  message_type_id distribution:')
    for mid, n in mt:
        print(f'    \"{mid}\": {n}')

    # Check if json_extract_string works on the msg column
    sample = con.execute(\"\"\"
        SELECT action, json_extract_string(msg, '\$[0]') as mt,
               json_extract_string(msg, '\$[3]') as payload
        FROM stg_ocpp_logs
        WHERE action = 'StartTransaction'
        LIMIT 1
    \"\"\").fetchall()
    if sample:
        print(f'  Sample StartTransaction parse:')
        print(f'    message_type_id: {sample[0][1]}')
        print(f'    payload: {str(sample[0][2])[:100]}')
    else:
        print(f'  No StartTransaction rows found in stg_ocpp_logs')

print()
print('=== DONE ===')
"
