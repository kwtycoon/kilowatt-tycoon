// kwwhat Analysis Engine (standalone)
//
// Lazy-loads DuckDB-WASM, feeds OCPP messages + ports through kwwhat's
// compiled SQL models, and returns charger reliability metrics.
//
// Dependencies:
//   - window.__kwtycoon_kwwhat_models (from kwwhat_models.js)
//   - DuckDB-WASM loaded from jsDelivr CDN on first call
//
// Usage:
//   var result = await window.kwwhatAnalyze(messages, ports);
//   // result.ok === true  -> result.metrics has the data
//   // result.ok === false -> result.error has the message

(function () {
  "use strict";

  var DUCKDB_CDN = "https://cdn.jsdelivr.net/npm/@duckdb/duckdb-wasm@1.29.0/+esm";
  var duckdbMod = null;
  var db = null;

  function messagesToCsv(messages) {
    var lines = ["timestamp,id,action,msg"];
    for (var i = 0; i < messages.length; i++) {
      var m = messages[i];
      var msg = (m.msg || "").replace(/"/g, '""');
      lines.push(
        (m.timestamp || "") + "," +
        (m.id || "") + "," +
        (m.action || "") + ',"' + msg + '"'
      );
    }
    return lines.join("\n");
  }

  function portsToCsv(ports) {
    var lines = [
      "charge_point_id,location_id,port_id,connector_id,connector_type,commissioned_ts,decommissioned_ts",
    ];
    for (var i = 0; i < ports.length; i++) {
      var p = ports[i];
      lines.push(
        (p.charge_point_id || "") + "," +
        (p.location_id || "") + "," +
        (p.port_id || "") + "," +
        (p.connector_id || "") + "," +
        (p.connector_type || "") + "," +
        (p.commissioned_ts || "") + ","
      );
    }
    return lines.join("\n");
  }

  async function ensureDuckDB() {
    if (db) return;
    duckdbMod = await import(DUCKDB_CDN);
    var bundles = duckdbMod.getJsDelivrBundles();
    var bundle = await duckdbMod.selectBundle(bundles);
    var worker = await duckdbMod.createWorker(bundle.mainWorker);
    var logger = new duckdbMod.ConsoleLogger(duckdbMod.LogLevel.WARNING);
    db = new duckdbMod.AsyncDuckDB(logger, worker);
    await db.instantiate(bundle.mainModule);
  }

  function pct(num, den) {
    if (Number(den) === 0) return "N/A";
    return (Number(num) / Number(den) * 100).toFixed(1) + "%";
  }

  window.kwwhatAnalyze = async function (messages, ports) {
    try {
      var models = window.__kwtycoon_kwwhat_models;
      if (!models || models.length === 0) {
        return { ok: false, error: "kwwhat_models.js not loaded" };
      }
      if (!messages || messages.length === 0) {
        return { ok: false, error: "No OCPP messages to analyze" };
      }

      await ensureDuckDB();

      var ocppCsv = messagesToCsv(messages);
      var portsCsv = portsToCsv(ports || []);

      await db.registerFileText("ocpp_logs.csv", ocppCsv);
      await db.registerFileText("ports.csv", portsCsv);

      var conn = await db.connect();
      try {
        try { await conn.query("ATTACH ':memory:' AS \"RAW\""); } catch (_) {}
        await conn.query('CREATE SCHEMA IF NOT EXISTS "RAW"."SEED"');
        await conn.query(
          'CREATE OR REPLACE TABLE "RAW"."SEED"."ocpp_1_6_synthetic_logs_14d" AS ' +
          "SELECT timestamp, id, action, replace(msg, '''', '\"') as msg " +
          "FROM read_csv('ocpp_logs.csv', auto_detect=true, header=true)"
        );
        await conn.query(
          'CREATE OR REPLACE TABLE "RAW"."SEED"."ports" AS ' +
          "SELECT * FROM read_csv('ports.csv', auto_detect=true, header=true)"
        );

        for (var i = 0; i < models.length; i++) {
          await conn.query(
            'CREATE OR REPLACE TABLE "' + models[i].name + '" AS (' + models[i].sql + ")"
          );
        }

        var metrics = {};

        var att = (await conn.query(
          "SELECT count(*) as total, count(*) FILTER (WHERE is_successful) as ok FROM fact_charge_attempts"
        )).toArray()[0];
        metrics.attemptTotal = Number(att.total);
        metrics.attemptSuccess = Number(att.ok);
        metrics.attemptSuccessRate = pct(att.ok, att.total);

        var vis = (await conn.query(
          "SELECT count(*) as total, count(*) FILTER (WHERE is_successful) as ok FROM fact_visits"
        )).toArray()[0];
        metrics.visitTotal = Number(vis.total);
        metrics.visitSuccess = Number(vis.ok);
        metrics.visitSuccessRate = pct(vis.ok, vis.total);

        var dt = (await conn.query(
          "SELECT type, round(sum(duration_minutes),1) as mins, count(*) as n FROM fact_downtime_daily GROUP BY type"
        )).toArray();
        metrics.downtime = [];
        for (var d = 0; d < dt.length; d++) {
          metrics.downtime.push({
            type: dt[d].type,
            minutes: Number(dt[d].mins),
            incidents: Number(dt[d].n),
          });
        }

        var tot = (await conn.query(
          "SELECT (SELECT count(*) FROM int_transactions) as txns, " +
          "(SELECT count(*) FROM int_status_changes) as statusChanges"
        )).toArray()[0];
        metrics.transactions = Number(tot.txns);
        metrics.statusChanges = Number(tot.statusChanges);
        metrics.messagesAnalyzed = messages.length;
        metrics.portsCount = (ports || []).length;

        return { ok: true, metrics: metrics };
      } finally {
        await conn.close();
      }
    } catch (err) {
      return { ok: false, error: err.message || String(err) };
    }
  };
})();
