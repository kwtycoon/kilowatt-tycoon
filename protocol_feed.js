// Protocol Feed Overlay
// Tabbed overlay showing OCPP and OpenADR message feeds side by side.
// Reads window.__kwtycoon_ocpp_feed, window.__kwtycoon_ocpp_ports,
// and window.__kwtycoon_openadr_feed set by Bevy feed systems.
// Toggle visibility with F6.

(function () {
  "use strict";

  var MAX_MESSAGES = 200;
  var visible = false;
  var container = null;
  var activeTab = "ocpp";

  // Per-tab state
  var tabs = {
    ocpp: { body: null, badge: null, msgCount: 0 },
    openadr: { body: null, badge: null, msgCount: 0 },
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
    container = document.createElement("div");
    container.id = "protocol-feed";
    container.style.cssText =
      "position:fixed;bottom:12px;right:12px;width:500px;height:360px;" +
      "z-index:50;background:rgba(10,10,20,0.92);border:1px solid #334155;" +
      "border-radius:8px;font-family:'SF Mono','Fira Code',monospace;" +
      "font-size:11px;color:#e0e0e0;display:none;flex-direction:column;" +
      "box-shadow:0 4px 24px rgba(0,0,0,0.5);pointer-events:auto;";

    // Header
    var header = document.createElement("div");
    header.style.cssText =
      "padding:6px 10px;border-bottom:1px solid #334155;display:flex;" +
      "justify-content:space-between;align-items:center;flex-shrink:0;";
    header.innerHTML =
      '<span style="color:#4ade80;font-weight:bold;">Protocol Feed</span>' +
      '<span style="color:#6b7280;font-size:10px;">F6 to toggle</span>';

    // Tab bar
    var tabBar = document.createElement("div");
    tabBar.style.cssText =
      "display:flex;border-bottom:1px solid #334155;flex-shrink:0;";

    function makeTab(id, label) {
      var btn = document.createElement("button");
      btn.textContent = label;
      btn.dataset.tab = id;
      btn.style.cssText =
        "flex:1;padding:5px 0;background:none;border:none;color:#94a3b8;" +
        "font-family:inherit;font-size:11px;cursor:pointer;border-bottom:2px solid transparent;";
      btn.addEventListener("click", function () {
        switchTab(id);
      });
      return btn;
    }

    var ocppTab = makeTab("ocpp", "OCPP");
    var openadrTab = makeTab("openadr", "OpenADR");
    tabBar.appendChild(ocppTab);
    tabBar.appendChild(openadrTab);

    // Badge containers inside tabs
    tabs.ocpp.badge = document.createElement("span");
    tabs.ocpp.badge.style.cssText =
      "background:#1e293b;color:#94a3b8;padding:1px 6px;border-radius:4px;" +
      "font-size:9px;margin-left:6px;";
    tabs.ocpp.badge.textContent = "0 ports";
    ocppTab.appendChild(tabs.ocpp.badge);

    tabs.openadr.badge = document.createElement("span");
    tabs.openadr.badge.style.cssText =
      "background:#1e293b;color:#94a3b8;padding:1px 6px;border-radius:4px;" +
      "font-size:9px;margin-left:6px;";
    tabs.openadr.badge.textContent = "DER";
    openadrTab.appendChild(tabs.openadr.badge);

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

    container.appendChild(header);
    container.appendChild(tabBar);
    container.appendChild(tabs.ocpp.body);
    container.appendChild(tabs.openadr.body);
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

  // ─── Polling ───

  function poll() {
    // Lazily create the overlay DOM on the first poll tick so messages
    // are captured from the very start. The overlay stays hidden until F6.
    if (!container) createOverlay();

    // OCPP feed
    var ocppFeed = window.__kwtycoon_ocpp_feed;
    if (ocppFeed && Array.isArray(ocppFeed) && ocppFeed.length > 0) {
      for (var i = 0; i < ocppFeed.length; i++) {
        addOcppMessage(ocppFeed[i]);
      }
      window.__kwtycoon_ocpp_feed = null;

      if (visible && activeTab === "ocpp") {
        tabs.ocpp.body.scrollTop = tabs.ocpp.body.scrollHeight;
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
      }
      window.__kwtycoon_openadr_feed = null;

      if (visible && activeTab === "openadr") {
        tabs.openadr.body.scrollTop = tabs.openadr.body.scrollHeight;
      }
    }

    requestAnimationFrame(poll);
  }

  function toggle() {
    visible = !visible;
    if (container) {
      container.style.display = visible ? "flex" : "none";
      if (visible) {
        var activeBody = tabs[activeTab].body;
        activeBody.scrollTop = activeBody.scrollHeight;
      }
    }
  }

  document.addEventListener("keydown", function (e) {
    if (e.key === "F6") {
      e.preventDefault();
      toggle();
    }
  });

  requestAnimationFrame(poll);
})();
