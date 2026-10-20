// The browser companion (iPhone and any phone without the app). Speaks the bridge protocol
// (protocol/schemas) over a WebSocket authenticated by an HttpOnly cookie set at pairing.
// No build step and no dependencies: the CSP allows only same-origin scripts.
(function () {
  "use strict";

  var VERSION = 1;
  var CHUNK = 192 * 1024; // Raw bytes per file.chunk, as on the PC and Android.
  var WINDOW = 4; // Unacknowledged chunks in flight.
  var MAX_RECEIVE = 1024 * 1024 * 1024; // A phone browser holds received files in memory.
  var PING_MS = 15000;
  var MAX_TEXT = 20000; // Characters per clipboard.text, as on the PC.

  var $ = function (id) {
    return document.getElementById(id);
  };
  var ws = null;
  var pingTimer = 0;
  var retryMs = 1000;
  var retryTimer = 0;
  var forgotten = false;
  var outgoing = {}; // transferId -> {file, row, acked, sent, accept, result}
  var incoming = {}; // transferId -> {offer, parts, hasher, received, next, row}
  var queue = Promise.resolve();

  // ---------- helpers ----------

  function uuid() {
    if (crypto.randomUUID) return crypto.randomUUID();
    var b = crypto.getRandomValues(new Uint8Array(16));
    b[6] = (b[6] & 15) | 64;
    b[8] = (b[8] & 63) | 128;
    var h = Array.prototype.map.call(b, function (x) {
      return (x + 256).toString(16).slice(1);
    }).join("");
    return h.slice(0, 8) + "-" + h.slice(8, 12) + "-" + h.slice(12, 16) + "-" + h.slice(16, 20) + "-" + h.slice(20);
  }

  function send(type, body) {
    if (!ws || ws.readyState !== 1) return false;
    ws.send(JSON.stringify({ v: VERSION, id: uuid(), type: type, ts: Date.now(), body: body }));
    return true;
  }

  function toBase64(bytes) {
    var s = "";
    for (var i = 0; i < bytes.length; i += 0x8000) {
      s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    }
    return btoa(s);
  }

  function fromBase64(text) {
    var s = atob(text);
    var out = new Uint8Array(s.length);
    for (var i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
    return out;
  }

  function readSlice(blob) {
    if (blob.arrayBuffer) return blob.arrayBuffer().then(function (b) { return new Uint8Array(b); });
    return new Promise(function (resolve, reject) {
      var r = new FileReader();
      r.onload = function () { resolve(new Uint8Array(r.result)); };
      r.onerror = function () { reject(r.error); };
      r.readAsArrayBuffer(blob);
    });
  }

  function size(bytes) {
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(0) + " KB";
    if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + " MB";
    return (bytes / 1024 / 1024 / 1024).toFixed(2) + " GB";
  }

  /** Same rules as the PC and Android: a plain file name, never a path. */
  function sanitize(name) {
    var base = String(name || "").split(/[\\/]/).pop();
    base = base.replace(/[\u0000-\u001f<>:"|?*]/g, "_").replace(/^[\s.]+|[\s.]+$/g, "");
    return base.slice(0, 120) || "file";
  }

  function deviceName() {
    var ua = navigator.userAgent;
    if (/iPhone/.test(ua)) return "iPhone";
    if (/iPad/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) return "iPad";
    if (/Android/.test(ua)) return "Android browser";
    return "Browser";
  }

  // ---------- screens ----------

  function setStatus(text, on) {
    var el = $("status");
    el.textContent = text;
    el.className = on ? "status on" : "status";
    $("send-label").className = on ? "send" : "send off";
    $("picker").disabled = !on;
    $("text-send").disabled = !on;
  }

  function showUnpaired(error) {
    forgotten = true;
    closeSocket();
    $("paired").hidden = true;
    $("unpaired").hidden = false;
    $("pair-error").hidden = !error;
    $("pair-error").textContent = error || "";
    setStatus("Not paired", false);
  }

  function showPaired(pcName) {
    forgotten = false;
    $("unpaired").hidden = true;
    $("paired").hidden = false;
    $("pc-name").textContent = pcName;
    var standalone = navigator.standalone || matchMedia("(display-mode: standalone)").matches;
    $("home-tip").hidden = standalone || !/iPhone|iPad/.test(navigator.userAgent);
  }

  // ---------- transfer rows ----------

  function addRow(direction, name, bytes) {
    var li = document.createElement("li");
    li.innerHTML =
      '<span class="arrow"></span><div class="file"><div class="name"></div>' +
      '<div class="detail"></div><div class="bar"><span></span></div></div>';
    li.querySelector(".arrow").textContent = direction === "out" ? "↑" : "↓";
    li.querySelector(".name").textContent = name;
    $("transfers").insertBefore(li, $("transfers").firstChild);
    var row = {
      li: li,
      progress: function (done) {
        var pct = bytes ? Math.min(100, Math.round((done / bytes) * 100)) : 100;
        li.querySelector(".bar span").style.width = pct + "%";
        li.querySelector(".detail").textContent = size(done) + " of " + size(bytes);
      },
      finish: function (ok, text) {
        li.className = ok ? "done" : "failed";
        li.querySelector(".arrow").textContent = ok ? "✓" : "!";
        li.querySelector(".detail").textContent = text;
        var bar = li.querySelector(".bar");
        if (bar) bar.remove();
      },
      action: function (label, onClick, ghost) {
        var button = document.createElement("button");
        button.type = "button";
        button.className = ghost ? "action ghost" : "action";
        button.textContent = label;
        button.onclick = onClick;
        li.appendChild(button);
        return button;
      },
    };
    row.progress(0);
    return row;
  }

  // ---------- text and links ----------

  /** The address, if the whole text is one plain web link (only those get an Open button). */
  function linkOf(text) {
    var t = text.trim();
    if (t.length > 2000 || /s/.test(t)) return null;
    try {
      var url = new URL(t);
      return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
    } catch (e) {
      return null;
    }
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text);
    }
    // Older browsers: copy from a hidden field.
    return new Promise(function (resolve, reject) {
      var field = document.createElement("textarea");
      field.value = text;
      field.setAttribute("readonly", "");
      field.style.position = "fixed";
      field.style.opacity = "0";
      document.body.appendChild(field);
      field.select();
      var ok = document.execCommand("copy");
      field.remove();
      if (ok) resolve();
      else reject(new Error("copy failed"));
    });
  }

  function addTextRow(direction, text) {
    var li = document.createElement("li");
    li.className = "note done";
    li.innerHTML =
      '<span class="arrow"></span><div class="file"><div class="name"></div>' +
      '<div class="detail"></div></div>';
    li.querySelector(".arrow").textContent = direction === "out" ? "↑" : "↓";
    li.querySelector(".name").textContent = text;
    li.querySelector(".detail").textContent =
      direction === "out" ? "Sent · copied on your PC" : "From your PC";
    if (direction === "in") {
      var copy = document.createElement("button");
      copy.type = "button";
      copy.className = "action";
      copy.textContent = "Copy";
      copy.onclick = function () {
        copyText(text).then(
          function () {
            copy.textContent = "Copied";
            setTimeout(function () { copy.textContent = "Copy"; }, 1500);
          },
          function () { copy.textContent = "Press and hold the text"; }
        );
      };
      li.appendChild(copy);
      var href = linkOf(text);
      if (href) {
        var open = document.createElement("a");
        open.className = "action ghost";
        open.textContent = "Open";
        open.href = href;
        open.target = "_blank";
        open.rel = "noopener noreferrer";
        li.appendChild(open);
      }
    }
    $("transfers").insertBefore(li, $("transfers").firstChild);
  }

  function onText(body) {
    if (typeof body.text !== "string" || !body.text.trim() || body.text.length > MAX_TEXT) return;
    addTextRow("in", body.text);
  }

  // ---------- sending ----------

  function sendFile(file) {
    var id = uuid();
    var row = addRow("out", file.name, file.size);
    var cancel = row.action("Cancel", function () {
      send("file.cancel", { transferId: id });
      settle(false, "Cancelled");
    }, true);
    var t = (outgoing[id] = { acked: -1, wake: null, accepted: null, result: null, failed: null });

    function settle(ok, text) {
      if (!outgoing[id]) return;
      delete outgoing[id];
      cancel.remove();
      row.finish(ok, text);
      if (t.wake) t.wake();
    }
    function waitFor(test) {
      return new Promise(function (resolve) {
        (function check() {
          if (!outgoing[id] || test()) return resolve();
          t.wake = check;
        })();
      });
    }

    row.li.querySelector(".detail").textContent = "Checking…";
    var hasher = new Sha256();
    var offset = 0;
    function hashNext() {
      if (offset >= file.size) return Promise.resolve(hasher.hex());
      return readSlice(file.slice(offset, offset + 4 * CHUNK)).then(function (bytes) {
        hasher.update(bytes);
        offset += bytes.length;
        return hashNext();
      });
    }

    return hashNext()
      .then(function (sha) {
        if (!outgoing[id]) return;
        var name = sanitize(file.name);
        if (!send("file.offer", { transferId: id, name: name, size: file.size, mime: file.type || undefined, sha256: sha })) {
          return settle(false, "Not connected");
        }
        row.li.querySelector(".detail").textContent = "Waiting for your PC…";
        return waitFor(function () { return t.accepted !== null; }).then(function () {
          if (!outgoing[id]) return;
          if (!t.accepted) return settle(false, t.failed || "Declined");
          var total = Math.ceil(file.size / CHUNK);
          var index = 0;
          function next() {
            if (!outgoing[id]) return;
            if (index >= total) {
              send("file.done", { transferId: id });
              return waitFor(function () { return t.result !== null; }).then(function () {
                if (t.result) settle(true, size(file.size) + " · sent");
                else settle(false, t.failed || "Not saved on your PC");
              });
            }
            return waitFor(function () { return index - t.acked <= WINDOW; }).then(function () {
              if (!outgoing[id]) return;
              return readSlice(file.slice(index * CHUNK, (index + 1) * CHUNK)).then(function (bytes) {
                if (!send("file.chunk", { transferId: id, index: index, data: toBase64(bytes) })) {
                  return settle(false, "Connection lost");
                }
                index++;
                row.progress(Math.min(file.size, index * CHUNK));
                return next();
              });
            });
          }
          return next();
        });
      })
      .catch(function () {
        send("file.cancel", { transferId: id });
        settle(false, "Couldn't read this file");
      });
  }

  function onOutgoing(type, body) {
    var t = outgoing[body.transferId];
    if (!t) return;
    if (type === "file.accept") t.accepted = true;
    else if (type === "file.decline") {
      t.accepted = false;
      t.failed = body.reason === "too_large" ? "Too large" : body.reason === "no_space" ? "PC is out of space" : "Declined";
    } else if (type === "file.ack") t.acked = Math.max(t.acked, body.index);
    else if (type === "file.result") {
      t.result = !!body.ok;
      if (!body.ok) t.failed = body.reason === "checksum" ? "Damaged on the way, try again" : "Not saved on your PC";
    } else if (type === "file.cancel") {
      t.accepted = false;
      t.result = false;
      t.failed = "Cancelled on your PC";
    }
    if (t.wake) {
      var wake = t.wake;
      t.wake = null;
      wake();
    }
  }

  // ---------- receiving ----------

  function onOffer(offer) {
    var id = offer.transferId;
    if (offer.size > MAX_RECEIVE || offer.size < 0) {
      send("file.decline", { transferId: id, reason: "too_large" });
      return;
    }
    var name = sanitize(offer.name);
    var row = addRow("in", name, offer.size);
    var r = (incoming[id] = { offer: offer, name: name, parts: [], hasher: new Sha256(), received: 0, next: 0, row: row });
    r.cancel = row.action("Cancel", function () {
      send("file.cancel", { transferId: id });
      dropIncoming(id, "Cancelled");
    }, true);
    send("file.accept", { transferId: id });
  }

  function dropIncoming(id, text) {
    var r = incoming[id];
    if (!r) return;
    delete incoming[id];
    r.cancel.remove();
    r.row.finish(false, text);
  }

  function onChunk(body) {
    var r = incoming[body.transferId];
    if (!r) return;
    if (body.index !== r.next) {
      send("file.result", { transferId: body.transferId, ok: false, reason: "size" });
      return dropIncoming(body.transferId, "Arrived out of order");
    }
    var bytes = fromBase64(body.data);
    r.received += bytes.length;
    if (r.received > r.offer.size) {
      send("file.result", { transferId: body.transferId, ok: false, reason: "size" });
      return dropIncoming(body.transferId, "Bigger than announced");
    }
    r.hasher.update(bytes);
    r.parts.push(bytes);
    r.next++;
    r.row.progress(r.received);
    send("file.ack", { transferId: body.transferId, index: body.index });
  }

  function onDone(body) {
    var id = body.transferId;
    var r = incoming[id];
    if (!r) return;
    if (r.received !== r.offer.size) {
      send("file.result", { transferId: id, ok: false, reason: "size" });
      return dropIncoming(id, "Incomplete");
    }
    if (r.hasher.hex() !== String(r.offer.sha256).toLowerCase()) {
      send("file.result", { transferId: id, ok: false, reason: "checksum" });
      return dropIncoming(id, "Damaged on the way");
    }
    send("file.result", { transferId: id, ok: true });
    delete incoming[id];
    r.cancel.remove();
    var type = r.offer.mime || "application/octet-stream";
    var file = new File(r.parts, r.name, { type: type });
    r.parts = null;
    r.row.finish(true, size(file.size) + " · from your PC");
    var url = URL.createObjectURL(file);
    // iOS: the share sheet offers Save Image / Save to Files. Elsewhere: a normal download.
    r.row.action("Save", function () {
      if (navigator.canShare && navigator.canShare({ files: [file] })) {
        navigator.share({ files: [file] }).catch(function () {});
      } else {
        var a = document.createElement("a");
        a.href = url;
        a.download = r.name;
        document.body.appendChild(a);
        a.click();
        a.remove();
      }
    });
  }

  // ---------- connection ----------

  function closeSocket() {
    clearInterval(pingTimer);
    clearTimeout(retryTimer);
    if (ws) {
      ws.onclose = null;
      ws.close();
      ws = null;
    }
  }

  function failAll(text) {
    Object.keys(outgoing).forEach(function (id) {
      onOutgoing("file.cancel", { transferId: id });
      var t = outgoing[id];
      if (t) t.failed = text;
    });
    Object.keys(incoming).forEach(function (id) {
      dropIncoming(id, text);
    });
  }

  function connect() {
    closeSocket();
    if (forgotten) return;
    setStatus("Connecting…", false);
    ws = new WebSocket("wss://" + location.host + "/ws");
    ws.onmessage = function (event) {
      var env;
      try {
        env = JSON.parse(event.data);
      } catch (e) {
        return;
      }
      var body = env.body || {};
      switch (env.type) {
        case "system.welcome":
          retryMs = 1000;
          showPaired(body.pcName);
          setStatus("Connected to " + body.pcName, true);
          break;
        case "device.unpair":
          showUnpaired("Your PC forgot this phone. Pair again to share files.");
          break;
        case "clipboard.text":
          onText(body);
          break;
        case "file.offer":
          onOffer(body);
          break;
        case "file.chunk":
          onChunk(body);
          break;
        case "file.done":
          onDone(body);
          break;
        case "file.cancel":
          if (incoming[body.transferId]) dropIncoming(body.transferId, "Cancelled on your PC");
          else onOutgoing(env.type, body);
          break;
        case "file.accept":
        case "file.decline":
        case "file.ack":
        case "file.result":
          onOutgoing(env.type, body);
          break;
      }
    };
    ws.onclose = function () {
      clearInterval(pingTimer);
      ws = null;
      failAll("Connection lost");
      setStatus("Reconnecting…", false);
      // A refused socket may mean the PC forgot us; /me tells.
      retryTimer = setTimeout(start, retryMs);
      retryMs = Math.min(retryMs * 2, 30000);
    };
    pingTimer = setInterval(function () {
      send("system.ping", {});
    }, PING_MS);
  }

  function start() {
    fetch("/me", { credentials: "same-origin", cache: "no-store" })
      .then(function (res) {
        if (res.status === 401) return showUnpaired();
        if (!res.ok) throw new Error("status " + res.status);
        return res.json().then(function (me) {
          showPaired(me.pcName);
          connect();
        });
      })
      .catch(function () {
        // PC asleep, Preku closed, or a different Wi-Fi: keep trying quietly.
        setStatus("Can't reach your PC", false);
        clearTimeout(retryTimer);
        retryTimer = setTimeout(start, retryMs);
        retryMs = Math.min(retryMs * 2, 30000);
      });
  }

  function pair(token) {
    setStatus("Pairing…", false);
    fetch("/pair", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token: token, deviceName: deviceName() }),
    })
      .then(function (res) {
        return res.json().then(function (reply) {
          if (!res.ok) {
            var why =
              reply.reason === "expired"
                ? "That code expired."
                : reply.reason === "not_pairing"
                  ? "Your PC isn't pairing right now."
                  : "That code was already used or isn't valid.";
            return showUnpaired(why + " Make a new one on your PC.");
          }
          showPaired(reply.pcName);
          connect();
        });
      })
      .catch(function () {
        showUnpaired("Couldn't reach your PC. Are both on the same Wi-Fi?");
      });
  }

  // ---------- wiring ----------

  $("picker").addEventListener("change", function (event) {
    var files = Array.prototype.slice.call(event.target.files || []);
    event.target.value = "";
    files.forEach(function (file) {
      queue = queue.then(function () {
        return sendFile(file);
      });
    });
  });

  $("text-form").addEventListener("submit", function (event) {
    event.preventDefault();
    var field = $("text");
    var text = field.value;
    if (!text.trim()) return;
    if (!send("clipboard.text", { text: text })) {
      setStatus("Not connected: the text wasn't sent", false);
      return;
    }
    addTextRow("out", text);
    field.value = "";
  });

  $("forget").addEventListener("click", function () {
    if (!confirm("Forget this PC? You'll need to scan a new code to share files again.")) return;
    send("device.unpair", {});
    showUnpaired();
  });

  document.addEventListener("visibilitychange", function () {
    // iOS suspends background pages; reconnect straight away when the user comes back.
    if (document.visibilityState === "visible" && !forgotten && (!ws || ws.readyState > 1)) {
      retryMs = 1000;
      clearTimeout(retryTimer);
      start();
    }
  });

  var match = /[#&]pair=([A-Za-z0-9_-]+)/.exec(location.hash);
  if (match) {
    history.replaceState(null, "", "/");
    pair(match[1]);
  } else {
    start();
  }
})();
