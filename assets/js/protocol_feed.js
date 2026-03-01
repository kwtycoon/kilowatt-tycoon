// Protocol Feed Overlay
// Tabbed overlay showing OCPP, OpenADR, and OCPI message feeds.
// Reads window.__kwtycoon_ocpp_feed, window.__kwtycoon_ocpp_ports,
// window.__kwtycoon_openadr_feed, and window.__kwtycoon_ocpi_feed
// set by Bevy feed systems.
// Toggle visibility via the "nerds" side tab or F6.

(function () {
  "use strict";

  var MAX_MESSAGES = 200;
  var MAX_BUFFER_BYTES = 16 * 1024 * 1024; // 16 MB circular buffer
  var visible = false;
  var container = null;
  var sideTab = null;
  var activeTab = "ocpp";

  // Circular buffers for export/analysis (shared 16 MB cap each)
  var buffers = {
    ocpp: { messages: [], bytes: 0 },
    openadr: { messages: [], bytes: 0 },
    ocpi: { messages: [], bytes: 0 },
  };

  function bufferPush(buf, entry) {
    var len = (entry.msg || "").length + (entry.timestamp || "").length + 100;
    buf.messages.push(entry);
    buf.bytes += len;
    while (buf.bytes > MAX_BUFFER_BYTES && buf.messages.length > 0) {
      var evicted = buf.messages.shift();
      buf.bytes -= (evicted.msg || "").length + (evicted.timestamp || "").length + 100;
    }
  }

  var analysisRunning = false;
  var analyzeBtn = null;
  var resultsPanel = null;

  // Per-tab state
  var tabs = {
    ocpp: { body: null, badge: null, msgCount: 0 },
    openadr: { body: null, badge: null, msgCount: 0 },
    ocpi: { body: null, badge: null, msgCount: 0 },
  };

  // OCPP action colors
  var OCPP_COLORS = {
    Heartbeat: "#6b7280",
    BootNotification: "#a78bfa",
    StatusNotification: "#60a5fa",
    StartTransaction: "#4ade80",
    StopTransaction: "#fb923c",
    MeterValues: "#22d3ee",
  };

  // OpenADR action colors (by action label)
  var OPENADR_COLORS = {
    "Solar VEN Register": "#a78bfa",
    "BESS VEN Register": "#a78bfa",
    "Solar Telemetry": "#fbbf24",
    "BESS Telemetry": "#22d3ee",
    "Grid Telemetry": "#60a5fa",
    "Price Signal": "#fb923c",
    "Demand Limit": "#f87171",
    "BESS DR Response": "#4ade80",
    "Solar Export Price": "#fbbf24",
    "Solar Export": "#facc15",
    "Customer Price Signal": "#a3e635",
  };

  // OCPI action colors
  var OCPI_COLORS = {
    "Location PUT": "#a78bfa",
    "Session PUT": "#4ade80",
    "Session PATCH": "#22d3ee",
    "CDR POST": "#fb923c",
    "EVSE Status": "#60a5fa",
    "Tariff PUT": "#e879f9",
  };

  // ─── OCPP detail extraction (unchanged from original) ───

  function extractOcppDetail(action, msg) {
    try {
      var arr = JSON.parse(msg);
      var payload = arr[3] || arr[2] || {};
      if (typeof payload === "string") payload = JSON.parse(payload);

      switch (action) {
        case "StatusNotification":
          return payload.status || "";
        case "StartTransaction":
          return "idTag=" + (payload.id_tag || payload.idTag || "?");
        case "StopTransaction":
          return (
            "txn=" + (payload.transaction_id || payload.transactionId || "?")
          );
        case "MeterValues":
          var sv =
            payload.meter_value &&
            payload.meter_value[0] &&
            payload.meter_value[0].sampled_value;
          if (!sv) {
            sv =
              payload.meterValue &&
              payload.meterValue[0] &&
              payload.meterValue[0].sampledValue;
          }
          if (sv && sv[0])
            return (
              (sv[0].measurand || "energy") + "=" + (sv[0].value || "?")
            );
          return "";
        case "BootNotification":
          return payload.charge_point_model || payload.chargePointModel || "";
        default:
          if (!action && arr[0] === 3) {
            if (payload.transaction_id != null || payload.transactionId != null)
              return (
                "txnId=" +
                (payload.transaction_id || payload.transactionId)
              );
            if (payload.status) return payload.status;
            if (payload.current_time || payload.currentTime) return "ok";
            return "ok";
          }
          return "";
      }
    } catch (_) {
      return "";
    }
  }

  // ─── OpenADR detail extraction ───

  function extractOpenAdrDetail(entry) {
    try {
      var obj = JSON.parse(entry.msg);
      var action = entry.action || "";

      if (action.indexOf("Register") !== -1) {
        return obj.ven_name || obj.venName || "";
      }
      if (action === "Price Signal") {
        var intervals = obj.intervals;
        if (intervals && intervals[0] && intervals[0].payloads) {
          var vals = intervals[0].payloads[0];
          if (vals && vals.values && vals.values[0] != null) {
            return "$" + Number(vals.values[0]).toFixed(3) + "/kWh";
          }
        }
        return obj.event_name || obj.eventName || "";
      }
      if (action === "Demand Limit") {
        return obj.event_name || obj.eventName || "";
      }
      if (action.indexOf("Telemetry") !== -1) {
        var resources = obj.resources;
        if (resources && resources[0] && resources[0].intervals) {
          var payloads = resources[0].intervals[0].payloads;
          if (payloads) {
            var parts = [];
            for (var i = 0; i < payloads.length && i < 2; i++) {
              var vt = payloads[i].value_type || payloads[i].valueType || "";
              var v = payloads[i].values && payloads[i].values[0];
              if (v != null) {
                if (typeof v === "number") v = v.toFixed(1);
                parts.push(vt + "=" + v);
              }
            }
            return parts.join(" ");
          }
        }
        return "";
      }
      if (action === "BESS DR Response") {
        var res = obj.resources;
        if (res && res[0] && res[0].intervals) {
          var p = res[0].intervals[0].payloads;
          if (p && p[0] && p[0].values && p[0].values[0] != null) {
            return Number(p[0].values[0]).toFixed(1) + " kW";
          }
        }
        return "";
      }
      if (action === "Solar Export Price") {
        var intervals = obj.intervals;
        if (intervals && intervals[0] && intervals[0].payloads) {
          var vals = intervals[0].payloads[0];
          if (vals && vals.values && vals.values[0] != null) {
            return "$" + Number(vals.values[0]).toFixed(3) + "/kWh";
          }
        }
        return obj.event_name || obj.eventName || "";
      }
      if (action === "Solar Export") {
        var intervals = obj.intervals;
        if (intervals && intervals[0] && intervals[0].payloads) {
          var vals = intervals[0].payloads[0];
          if (vals && vals.values && vals.values[0] != null) {
            return Number(vals.values[0]).toFixed(1) + " kW";
          }
        }
        return obj.event_name || obj.eventName || "";
      }
      if (action === "Customer Price Signal") {
        var name = obj.event_name || obj.eventName || "";
        var intervals = obj.intervals;
        if (intervals && intervals[0] && intervals[0].payloads) {
          var vals = intervals[0].payloads[0];
          if (vals && vals.values && vals.values[0] != null) {
            var mode = name.replace("CustomerPrice-", "").replace(/-\d+$/, "");
            return "$" + Number(vals.values[0]).toFixed(2) + "/kWh (" + mode + ")";
          }
        }
        return name;
      }
      return "";
    } catch (_) {
      return "";
    }
  }

  // ─── OCPI detail extraction ───

  function extractOcpiDetail(entry) {
    try {
      var obj = JSON.parse(entry.msg);
      var action = entry.action || "";

      if (action === "Location PUT") {
        var name = obj.name || "";
        var evseCount = (obj.evses && obj.evses.length) || 0;
        return name + (evseCount ? " (" + evseCount + " EVSE)" : "");
      }
      if (action === "Session PUT") {
        return "id=" + (obj.id || "?") + " evse=" + (obj.evse_uid || "?");
      }
      if (action === "Session PATCH") {
        var kwh = obj.kwh != null ? Number(obj.kwh).toFixed(1) + "kWh" : "";
        var cost = obj.total_cost && obj.total_cost.before_taxes != null
          ? " $" + Number(obj.total_cost.before_taxes).toFixed(2)
          : "";
        return kwh + cost;
      }
      if (action === "CDR POST") {
        var e = obj.total_energy != null ? Number(obj.total_energy).toFixed(1) + "kWh" : "";
        var c = obj.total_cost && obj.total_cost.before_taxes != null
          ? " $" + Number(obj.total_cost.before_taxes).toFixed(2)
          : "";
        var t = obj.total_time != null ? " " + Number(obj.total_time * 60).toFixed(0) + "min" : "";
        return e + c + t;
      }
      if (action === "EVSE Status") {
        return (obj.status || "") + " " + (obj.evse_uid || "");
      }
      if (action === "Tariff PUT") {
        var altText = obj.tariff_alt_text || obj.tariffAltText;
        if (altText && altText[0] && altText[0].text) {
          return altText[0].text;
        }
        var elems = obj.elements;
        if (elems && elems[0] && elems[0].price_components) {
          var pc = elems[0].price_components[0];
          if (pc) {
            return "Energy: $" + Number(pc.price).toFixed(2) + "/kWh";
          }
        }
        return obj.id || "";
      }
      return "";
    } catch (_) {
      return "";
    }
  }

  function formatTime(ts) {
    try {
      var d = new Date(ts);
      return (
        String(d.getHours()).padStart(2, "0") +
        ":" +
        String(d.getMinutes()).padStart(2, "0") +
        ":" +
        String(d.getSeconds()).padStart(2, "0")
      );
    } catch (_) {
      return ts.substring(11, 19);
    }
  }

  // ─── Overlay construction ───

  function createOverlay() {
    // Side tab — always visible, 60% down the right edge
    sideTab = document.createElement("div");
    sideTab.id = "nerd-curtain-tab";
    sideTab.style.cssText =
      "position:fixed;right:0;bottom:80px;" +
      "writing-mode:vertical-rl;text-orientation:mixed;" +
      "padding:10px 5px;background:rgba(10,10,20,0.85);" +
      "border:1px solid #334155;border-right:none;border-radius:8px 0 0 8px;" +
      "color:#4ade80;font-family:'SF Mono','Fira Code',monospace;" +
      "font-size:12px;font-weight:bold;cursor:pointer;z-index:51;" +
      "letter-spacing:2px;pointer-events:auto;user-select:none;" +
      "transition:background 0.2s,border-color 0.2s;";
    sideTab.textContent = "\uD83E\uDD13 nerds";
    sideTab.addEventListener("click", function () { toggle(); });
    sideTab.addEventListener("mouseenter", function () {
      sideTab.style.background = "rgba(10,10,20,0.95)";
      sideTab.style.borderColor = "#4ade80";
    });
    sideTab.addEventListener("mouseleave", function () {
      if (!visible) {
        sideTab.style.background = "rgba(10,10,20,0.85)";
        sideTab.style.borderColor = "#334155";
      }
    });
    document.body.appendChild(sideTab);

    container = document.createElement("div");
    container.id = "protocol-feed";
    container.style.cssText =
      "position:fixed;bottom:12px;right:12px;width:500px;height:360px;" +
      "z-index:50;background:rgba(10,10,20,0.92);border:1px solid #334155;" +
      "border-radius:8px;font-family:'SF Mono','Fira Code',monospace;" +
      "font-size:11px;color:#e0e0e0;display:flex;flex-direction:column;" +
      "box-shadow:0 4px 24px rgba(0,0,0,0.5);pointer-events:auto;" +
      "transition:transform 0.3s ease,opacity 0.3s ease;" +
      "transform:translateX(calc(100% + 24px));opacity:0;";

    // Header
    var header = document.createElement("div");
    header.style.cssText =
      "padding:6px 10px;border-bottom:1px solid #334155;display:flex;" +
      "justify-content:space-between;align-items:center;flex-shrink:0;";
    header.innerHTML =
      '<span style="color:#4ade80;font-weight:bold;">Protocol Feed</span>';

    // Tab bar
    var tabBar = document.createElement("div");
    tabBar.style.cssText =
      "display:flex;border-bottom:1px solid #334155;flex-shrink:0;";

    function makeTab(id, label) {
      var btn = document.createElement("button");
      btn.dataset.tab = id;
      btn.style.cssText =
        "flex:1;padding:5px 8px;background:none;border:none;color:#94a3b8;" +
        "font-family:inherit;font-size:11px;cursor:pointer;border-bottom:2px solid transparent;" +
        "display:flex;align-items:center;gap:6px;";
      var lbl = document.createElement("span");
      lbl.textContent = label;
      btn.appendChild(lbl);
      btn.addEventListener("click", function () {
        switchTab(id);
      });
      return btn;
    }

    var ocppTab = makeTab("ocpp", "OCPP");
    var openadrTab = makeTab("openadr", "OpenADR");
    var ocpiTab = makeTab("ocpi", "OCPI");
    tabBar.appendChild(ocppTab);
    tabBar.appendChild(openadrTab);
    tabBar.appendChild(ocpiTab);

    // Badge containers inside tabs
    tabs.ocpp.badge = document.createElement("span");
    tabs.ocpp.badge.style.cssText =
      "background:#1e293b;color:#94a3b8;padding:1px 6px;border-radius:4px;" +
      "font-size:9px;";
    tabs.ocpp.badge.textContent = "0 ports";
    ocppTab.appendChild(tabs.ocpp.badge);

    var exportBtn = document.createElement("button");
    exportBtn.textContent = "\u2913";
    exportBtn.title = "Export OCPP data (CSV)";
    exportBtn.style.cssText =
      "background:#1e293b;color:#94a3b8;border:1px solid #334155;padding:1px 6px;" +
      "border-radius:4px;font-size:11px;cursor:pointer;margin-left:auto;";
    exportBtn.addEventListener("click", function () { exportFeed("ocpp"); });
    ocppTab.appendChild(exportBtn);

    tabs.openadr.badge = document.createElement("span");
    tabs.openadr.badge.style.cssText =
      "background:#1e293b;color:#94a3b8;padding:1px 6px;border-radius:4px;" +
      "font-size:9px;";
    tabs.openadr.badge.textContent = "DER";
    openadrTab.appendChild(tabs.openadr.badge);

    var exportAdrBtn = document.createElement("button");
    exportAdrBtn.textContent = "\u2913";
    exportAdrBtn.title = "Export OpenADR data (CSV)";
    exportAdrBtn.style.cssText =
      "background:#1e293b;color:#94a3b8;border:1px solid #334155;padding:1px 6px;" +
      "border-radius:4px;font-size:11px;cursor:pointer;margin-left:auto;";
    exportAdrBtn.addEventListener("click", function () { exportFeed("openadr"); });
    openadrTab.appendChild(exportAdrBtn);

    tabs.ocpi.badge = document.createElement("span");
    tabs.ocpi.badge.style.cssText =
      "background:#1e293b;color:#94a3b8;padding:1px 6px;border-radius:4px;" +
      "font-size:9px;";
    tabs.ocpi.badge.textContent = "Roaming";
    ocpiTab.appendChild(tabs.ocpi.badge);

    var exportOcpiBtn = document.createElement("button");
    exportOcpiBtn.textContent = "\u2913";
    exportOcpiBtn.title = "Export OCPI data (CSV)";
    exportOcpiBtn.style.cssText =
      "background:#1e293b;color:#94a3b8;border:1px solid #334155;padding:1px 6px;" +
      "border-radius:4px;font-size:11px;cursor:pointer;margin-left:auto;";
    exportOcpiBtn.addEventListener("click", function () { exportFeed("ocpi"); });
    ocpiTab.appendChild(exportOcpiBtn);

    // kwwhat row (visible only on OCPP tab)
    var kwwhatRow = document.createElement("div");
    kwwhatRow.id = "kwwhat-row";
    kwwhatRow.style.cssText =
      "display:flex;align-items:center;padding:4px 10px;border-bottom:1px solid #334155;" +
      "flex-shrink:0;gap:6px;font-size:10px;";

    var kwwhatIcon = document.createElement("span");
    kwwhatIcon.textContent = "\uD83D\uDCC8";
    kwwhatIcon.style.cssText = "font-size:11px;";
    kwwhatRow.appendChild(kwwhatIcon);

    var kwwhatLabel = document.createElement("span");
    kwwhatLabel.textContent = "kwwhat";
    kwwhatLabel.style.cssText = "color:#94a3b8;font-weight:bold;letter-spacing:0.5px;";
    kwwhatRow.appendChild(kwwhatLabel);

    analyzeBtn = document.createElement("button");
    analyzeBtn.textContent = "Run";
    analyzeBtn.disabled = true;
    analyzeBtn.style.cssText =
      "background:linear-gradient(90deg,#4ade80,#22d3ee);color:#0a0a14;border:none;padding:2px 10px;" +
      "border-radius:4px;font-family:inherit;font-size:9px;font-weight:bold;cursor:pointer;" +
      "margin-left:auto;opacity:0.5;letter-spacing:0.5px;transition:transform 0.1s;";
    analyzeBtn.addEventListener("mouseenter", function () { if (!analyzeBtn.disabled) analyzeBtn.style.transform = "scale(1.1)"; });
    analyzeBtn.addEventListener("mouseleave", function () { analyzeBtn.style.transform = "scale(1)"; });
    analyzeBtn.addEventListener("click", runAnalysis);
    kwwhatRow.appendChild(analyzeBtn);

    // Message bodies
    function makeBody() {
      var body = document.createElement("div");
      body.style.cssText =
        "flex:1;overflow-y:auto;padding:4px 0;scrollbar-width:thin;" +
        "scrollbar-color:#334155 transparent;display:none;";
      return body;
    }

    tabs.ocpp.body = makeBody();
    tabs.openadr.body = makeBody();
    tabs.ocpi.body = makeBody();

    container.appendChild(header);
    container.appendChild(tabBar);
    container.appendChild(kwwhatRow);
    container.appendChild(tabs.ocpp.body);
    container.appendChild(tabs.openadr.body);
    container.appendChild(tabs.ocpi.body);
    document.body.appendChild(container);

    switchTab("ocpp");
  }

  function switchTab(id) {
    activeTab = id;
    var btns = container.querySelectorAll("button[data-tab]");
    for (var i = 0; i < btns.length; i++) {
      var isActive = btns[i].dataset.tab === id;
      btns[i].style.color = isActive ? "#e0e0e0" : "#94a3b8";
      btns[i].style.borderBottomColor = isActive ? "#4ade80" : "transparent";
      btns[i].style.fontWeight = isActive ? "bold" : "normal";
    }
    tabs.ocpp.body.style.display = id === "ocpp" ? "block" : "none";
    tabs.openadr.body.style.display = id === "openadr" ? "block" : "none";
    tabs.ocpi.body.style.display = id === "ocpi" ? "block" : "none";

    var kwr = container && container.querySelector("#kwwhat-row");
    if (kwr) kwr.style.display = id === "ocpp" ? "flex" : "none";

    var activeBody = tabs[id].body;
    activeBody.scrollTop = activeBody.scrollHeight;
  }

  // ─── Message rendering ───

  function addOcppMessage(entry) {
    var el = document.createElement("div");
    el.style.cssText =
      "padding:1px 10px;line-height:1.5;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;";

    var action = entry.action || "";
    var isCallResult = !action && entry.msg && entry.msg.charAt(1) === "3";

    var color =
      OCPP_COLORS[action] || (isCallResult ? "#4b5563" : "#9ca3af");
    var label = action || (isCallResult ? "CallResult" : "???");
    var detail = extractOcppDetail(action, entry.msg);

    var isFaulted =
      action === "StatusNotification" && detail.indexOf("Faulted") !== -1;
    if (isFaulted) color = "#f87171";

    el.innerHTML =
      '<span style="color:#6b7280;">' +
      formatTime(entry.timestamp) +
      "</span> " +
      '<span style="color:' +
      color +
      ";font-weight:" +
      (isCallResult ? "normal" : "bold") +
      ';">' +
      label +
      "</span> " +
      '<span style="color:#94a3b8;">' +
      (entry.id || "") +
      "</span>" +
      (detail
        ? ' <span style="color:#cbd5e1;">' + detail + "</span>"
        : "");

    tabs.ocpp.body.appendChild(el);
    tabs.ocpp.msgCount++;

    while (tabs.ocpp.msgCount > MAX_MESSAGES) {
      if (tabs.ocpp.body.firstChild) tabs.ocpp.body.removeChild(tabs.ocpp.body.firstChild);
      tabs.ocpp.msgCount--;
    }
  }

  function addOpenAdrMessage(entry) {
    var el = document.createElement("div");
    el.style.cssText =
      "padding:1px 10px;line-height:1.5;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;";

    var action = entry.action || "";
    var color = OPENADR_COLORS[action] || "#9ca3af";
    var msgType = entry.message_type || "";
    var detail = extractOpenAdrDetail(entry);

    el.innerHTML =
      '<span style="color:#6b7280;">' +
      formatTime(entry.timestamp) +
      "</span> " +
      '<span style="color:' +
      color +
      ';font-weight:bold;">' +
      action +
      "</span> " +
      '<span style="color:#64748b;font-size:10px;">[' +
      msgType +
      "]</span> " +
      '<span style="color:#94a3b8;">' +
      (entry.ven_id || "") +
      "</span>" +
      (detail
        ? ' <span style="color:#cbd5e1;">' + detail + "</span>"
        : "");

    tabs.openadr.body.appendChild(el);
    tabs.openadr.msgCount++;

    while (tabs.openadr.msgCount > MAX_MESSAGES) {
      if (tabs.openadr.body.firstChild)
        tabs.openadr.body.removeChild(tabs.openadr.body.firstChild);
      tabs.openadr.msgCount--;
    }
  }

  function addOcpiMessage(entry) {
    var el = document.createElement("div");
    el.style.cssText =
      "padding:1px 10px;line-height:1.5;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;";

    var action = entry.action || "";
    var color = OCPI_COLORS[action] || "#9ca3af";
    var objType = entry.object_type || "";
    var detail = extractOcpiDetail(entry);

    el.innerHTML =
      '<span style="color:#6b7280;">' +
      formatTime(entry.timestamp) +
      "</span> " +
      '<span style="color:' +
      color +
      ';font-weight:bold;">' +
      action +
      "</span> " +
      '<span style="color:#64748b;font-size:10px;">[' +
      objType +
      "]</span>" +
      (detail
        ? ' <span style="color:#cbd5e1;">' + detail + "</span>"
        : "");

    tabs.ocpi.body.appendChild(el);
    tabs.ocpi.msgCount++;

    while (tabs.ocpi.msgCount > MAX_MESSAGES) {
      if (tabs.ocpi.body.firstChild)
        tabs.ocpi.body.removeChild(tabs.ocpi.body.firstChild);
      tabs.ocpi.msgCount--;
    }
  }

  // ─── Polling ───

  function poll() {
    // Lazily create the overlay DOM on the first poll tick so messages
    // are captured from the very start.
    if (!container) createOverlay();

    // OCPP feed
    var ocppFeed = window.__kwtycoon_ocpp_feed;
    if (ocppFeed && Array.isArray(ocppFeed) && ocppFeed.length > 0) {
      for (var i = 0; i < ocppFeed.length; i++) {
        addOcppMessage(ocppFeed[i]);
        bufferPush(buffers.ocpp, ocppFeed[i]);
      }
      window.__kwtycoon_ocpp_feed = null;

      if (visible && activeTab === "ocpp") {
        tabs.ocpp.body.scrollTop = tabs.ocpp.body.scrollHeight;
      }
      if (analyzeBtn) {
        analyzeBtn.disabled = false;
        analyzeBtn.style.opacity = "1";
      }
    }

    // OCPP ports badge
    var ports = window.__kwtycoon_ocpp_ports;
    if (ports && Array.isArray(ports) && tabs.ocpp.badge) {
      tabs.ocpp.badge.textContent =
        ports.length + (ports.length === 1 ? " port" : " ports");
    }

    // OpenADR feed
    var adrFeed = window.__kwtycoon_openadr_feed;
    if (adrFeed && Array.isArray(adrFeed) && adrFeed.length > 0) {
      for (var j = 0; j < adrFeed.length; j++) {
        addOpenAdrMessage(adrFeed[j]);
        bufferPush(buffers.openadr, adrFeed[j]);
      }
      window.__kwtycoon_openadr_feed = null;

      if (visible && activeTab === "openadr") {
        tabs.openadr.body.scrollTop = tabs.openadr.body.scrollHeight;
      }
    }

    // OCPI feed
    var ocpiFeed = window.__kwtycoon_ocpi_feed;
    if (ocpiFeed && Array.isArray(ocpiFeed) && ocpiFeed.length > 0) {
      for (var k = 0; k < ocpiFeed.length; k++) {
        addOcpiMessage(ocpiFeed[k]);
        bufferPush(buffers.ocpi, ocpiFeed[k]);
      }
      window.__kwtycoon_ocpi_feed = null;

      if (visible && activeTab === "ocpi") {
        tabs.ocpi.body.scrollTop = tabs.ocpi.body.scrollHeight;
      }
    }

    requestAnimationFrame(poll);
  }

  function toggle() {
    visible = !visible;
    if (container) {
      container.style.transform = visible ? "translateX(0)" : "translateX(calc(100% + 24px))";
      container.style.opacity = visible ? "1" : "0";
      if (visible) {
        var activeBody = tabs[activeTab].body;
        activeBody.scrollTop = activeBody.scrollHeight;
      }
    }
    if (sideTab) {
      sideTab.style.borderColor = visible ? "#4ade80" : "#334155";
      sideTab.style.background = visible ? "rgba(10,10,20,0.95)" : "rgba(10,10,20,0.85)";
    }
  }

  document.addEventListener("keydown", function (e) {
    if (e.key === "F6") {
      e.preventDefault();
      toggle();
    }
  });

  // ─── kwwhat Analysis (delegates to kwwhat_analyze.js) ───

  function showResults(view) {
    if (!resultsPanel) {
      resultsPanel = document.createElement("div");
      resultsPanel.style.cssText =
        "position:absolute;top:0;left:0;right:0;bottom:0;background:rgba(10,10,20,0.97);" +
        "border-radius:8px;display:flex;flex-direction:column;overflow:hidden;z-index:10;";
      container.appendChild(resultsPanel);
    }
    resultsPanel.innerHTML = "";
    resultsPanel.style.display = "flex";

    var hdr = document.createElement("div");
    hdr.style.cssText =
      "padding:8px 12px;border-bottom:1px solid #334155;display:flex;" +
      "justify-content:space-between;align-items:center;flex-shrink:0;";
    hdr.innerHTML = '<span style="color:#4ade80;font-weight:bold;font-size:12px;">kwwhat Analysis</span>';

    var closeBtn = document.createElement("button");
    closeBtn.textContent = "Back to Feed";
    closeBtn.style.cssText =
      "background:none;border:1px solid #334155;color:#94a3b8;padding:2px 8px;" +
      "border-radius:4px;font-family:inherit;font-size:10px;cursor:pointer;";
    closeBtn.addEventListener("click", function () {
      resultsPanel.style.display = "none";
    });
    hdr.appendChild(closeBtn);

    var body = document.createElement("div");
    body.style.cssText = "flex:1;overflow-y:auto;padding:8px 12px;";
    body.innerHTML = view;

    resultsPanel.appendChild(hdr);
    resultsPanel.appendChild(body);
  }

  function metricRow(label, value) {
    return (
      '<div style="display:flex;justify-content:space-between;padding:4px 0;border-bottom:1px solid #1e293b;">' +
      '<span style="color:#94a3b8;font-size:11px;">' + label + "</span>" +
      '<span style="color:#c084fc;font-weight:bold;font-size:12px;">' + value + "</span>" +
      "</div>"
    );
  }

  function renderMetrics(m) {
    var html = "";
    html += metricRow("Charge Attempt Success", m.attemptSuccessRate + " (" + m.attemptSuccess + "/" + m.attemptTotal + ")");
    html += metricRow("Visit Success", m.visitSuccessRate + " (" + m.visitSuccess + "/" + m.visitTotal + ")");
    for (var i = 0; i < m.downtime.length; i++) {
      var d = m.downtime[i];
      html += metricRow(d.type + " Downtime", d.minutes + " min (" + d.incidents + ")");
    }
    html += metricRow("Transactions", String(m.transactions));
    html += metricRow("Status Changes", String(m.statusChanges));
    html += '<div style="color:#6b7280;font-size:9px;margin-top:8px;text-align:center;">' +
      m.messagesAnalyzed + " messages analyzed &middot; " + m.portsCount + " ports</div>";
    showResults(html);
  }

  async function runAnalysis() {
    if (analysisRunning || buffers.ocpp.messages.length === 0) return;
    if (typeof window.kwwhatAnalyze !== "function") {
      showResults('<div style="color:#f87171;padding:20px 0;text-align:center;">kwwhat_analyze.js not loaded</div>');
      return;
    }

    analysisRunning = true;
    analyzeBtn.textContent = "Running\u2026";
    analyzeBtn.disabled = true;
    analyzeBtn.style.opacity = "0.6";

    showResults(
      '<div style="color:#94a3b8;padding:20px 0;text-align:center;">' +
      '<div style="margin-bottom:8px;">Loading DuckDB-WASM...</div>' +
      '<div style="font-size:10px;color:#6b7280;">First load fetches ~10MB from CDN</div></div>'
    );

    var result = await window.kwwhatAnalyze(buffers.ocpp.messages, window.__kwtycoon_ocpp_ports || []);

    if (result.ok) {
      renderMetrics(result.metrics);
    } else {
      showResults(
        '<div style="color:#f87171;padding:12px 0;">' +
        '<div style="font-weight:bold;margin-bottom:4px;">Analysis failed</div>' +
        '<div style="font-size:10px;word-break:break-all;">' + result.error + "</div></div>"
      );
    }

    analysisRunning = false;
    if (analyzeBtn) {
      analyzeBtn.textContent = "Run";
      analyzeBtn.disabled = buffers.ocpp.messages.length === 0;
      analyzeBtn.style.opacity = buffers.ocpp.messages.length === 0 ? "0.5" : "1";
    }
  }

  // ─── Data export ───

  function downloadFile(filename, content, mime) {
    var blob = new Blob([content], { type: mime || "text/plain" });
    var a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = filename;
    a.click();
    URL.revokeObjectURL(a.href);
  }

  function arrayToCsv(headers, rows, fieldGetter) {
    var lines = [headers.join(",")];
    for (var i = 0; i < rows.length; i++) {
      var vals = fieldGetter(rows[i]);
      for (var j = 0; j < vals.length; j++) {
        var v = vals[j] || "";
        if (v.indexOf(",") !== -1 || v.indexOf('"') !== -1 || v.indexOf("\n") !== -1) {
          vals[j] = '"' + v.replace(/"/g, '""') + '"';
        }
      }
      lines.push(vals.join(","));
    }
    return lines.join("\n");
  }

  function exportFeed(tabId) {
    var buf = buffers[tabId];
    if (!buf || buf.messages.length === 0) return;

    if (tabId === "ocpp") {
      downloadFile("ocpp_messages.csv", arrayToCsv(
        ["timestamp", "id", "action", "msg"],
        buf.messages,
        function (m) { return [m.timestamp, m.id, m.action, m.msg]; }
      ), "text/csv");

      var ports = window.__kwtycoon_ocpp_ports || [];
      if (ports.length > 0) {
        downloadFile("ocpp_ports.csv", arrayToCsv(
          ["charge_point_id", "location_id", "port_id", "connector_id", "connector_type", "commissioned_ts", "decommissioned_ts"],
          ports,
          function (p) { return [p.charge_point_id, p.location_id, p.port_id, p.connector_id, p.connector_type, p.commissioned_ts || "", ""]; }
        ), "text/csv");
      }
    } else if (tabId === "openadr") {
      downloadFile("openadr_messages.csv", arrayToCsv(
        ["timestamp", "ven_id", "message_type", "action", "msg"],
        buf.messages,
        function (m) { return [m.timestamp, m.ven_id, m.message_type, m.action, m.msg]; }
      ), "text/csv");
    } else if (tabId === "ocpi") {
      downloadFile("ocpi_messages.csv", arrayToCsv(
        ["timestamp", "party_id", "object_type", "action", "msg"],
        buf.messages,
        function (m) { return [m.timestamp, m.party_id, m.object_type, m.action, m.msg]; }
      ), "text/csv");
    }
  }

  requestAnimationFrame(poll);
})();
