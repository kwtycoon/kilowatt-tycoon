// OCPP Live Feed Overlay
// Reads window.__kwtycoon_ocpp_feed (incremental message batches)
// and window.__kwtycoon_ocpp_ports (port registry) set by the Bevy
// ocpp_feed_system, then renders a scrolling log overlay on top of
// the game canvas. Toggle visibility with F6.

(function () {
  "use strict";

  var MAX_MESSAGES = 200;
  var visible = false;
  var container = null;
  var body = null;
  var badge = null;
  var msgCount = 0;

  var ACTION_COLORS = {
    Heartbeat: "#6b7280",
    BootNotification: "#a78bfa",
    StatusNotification: "#60a5fa",
    StartTransaction: "#4ade80",
    StopTransaction: "#fb923c",
    MeterValues: "#22d3ee",
  };

  function extractDetail(action, msg) {
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

  function createOverlay() {
    container = document.createElement("div");
    container.id = "ocpp-feed";
    container.style.cssText =
      "position:fixed;bottom:12px;right:12px;width:460px;height:320px;" +
      "z-index:50;background:rgba(10,10,20,0.92);border:1px solid #334155;" +
      "border-radius:8px;font-family:'SF Mono','Fira Code',monospace;" +
      "font-size:11px;color:#e0e0e0;display:none;flex-direction:column;" +
      "box-shadow:0 4px 24px rgba(0,0,0,0.5);pointer-events:auto;";

    var header = document.createElement("div");
    header.style.cssText =
      "padding:6px 10px;border-bottom:1px solid #334155;display:flex;" +
      "justify-content:space-between;align-items:center;flex-shrink:0;";
    header.innerHTML =
      '<span style="color:#4ade80;font-weight:bold;">OCPP Feed</span>' +
      '<span style="color:#6b7280;font-size:10px;">F6 to toggle</span>';

    badge = document.createElement("span");
    badge.style.cssText =
      "background:#1e293b;color:#94a3b8;padding:1px 6px;border-radius:4px;" +
      "font-size:10px;margin-left:8px;";
    badge.textContent = "0 ports";
    header.firstChild.appendChild(badge);

    body = document.createElement("div");
    body.style.cssText =
      "flex:1;overflow-y:auto;padding:4px 0;scrollbar-width:thin;" +
      "scrollbar-color:#334155 transparent;";

    container.appendChild(header);
    container.appendChild(body);
    document.body.appendChild(container);
  }

  function addMessage(entry) {
    var el = document.createElement("div");
    el.style.cssText = "padding:1px 10px;line-height:1.5;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;";

    var action = entry.action || "";
    var isCallResult = !action && entry.msg && entry.msg.charAt(1) === "3";

    var color = ACTION_COLORS[action] || (isCallResult ? "#4b5563" : "#9ca3af");
    var label = action || (isCallResult ? "CallResult" : "???");
    var detail = extractDetail(action, entry.msg);

    var isFaulted =
      action === "StatusNotification" && detail.indexOf("Faulted") !== -1;
    if (isFaulted) color = "#f87171";

    el.innerHTML =
      '<span style="color:#6b7280;">' +
      formatTime(entry.timestamp) +
      "</span> " +
      '<span style="color:' +
      color +
      ';font-weight:' +
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

    body.appendChild(el);
    msgCount++;

    while (msgCount > MAX_MESSAGES) {
      if (body.firstChild) body.removeChild(body.firstChild);
      msgCount--;
    }
  }

  function poll() {
    var feed = window.__kwtycoon_ocpp_feed;
    if (feed && Array.isArray(feed) && feed.length > 0) {
      for (var i = 0; i < feed.length; i++) {
        addMessage(feed[i]);
      }
      window.__kwtycoon_ocpp_feed = null;

      if (visible) {
        body.scrollTop = body.scrollHeight;
      }
    }

    var ports = window.__kwtycoon_ocpp_ports;
    if (ports && Array.isArray(ports) && badge) {
      badge.textContent = ports.length + (ports.length === 1 ? " port" : " ports");
    }

    requestAnimationFrame(poll);
  }

  function toggle() {
    visible = !visible;
    if (container) {
      container.style.display = visible ? "flex" : "none";
      if (visible) {
        body.scrollTop = body.scrollHeight;
      }
    }
  }

  document.addEventListener("keydown", function (e) {
    if (e.key === "F6") {
      e.preventDefault();
      if (!container) createOverlay();
      toggle();
    }
  });

  requestAnimationFrame(poll);
})();
