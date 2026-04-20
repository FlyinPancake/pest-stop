var Clay = require("./clay");
var clayConfig = require("./config.json");
var clay = new Clay(clayConfig, null, { autoHandleEvents: false });

var DEFAULT_SETTINGS = {
  backend_url: "https://pest.hoth.froggo.boo",
  nearby_limit: 5,
  departure_limit: 3
};

const CMD_REQUEST_DEPARTURES = 0;
const CMD_DEPARTURE_DATA = 1;
const CMD_STATUS = 2;
const CMD_REQUEST_NEARBY = 3;
const CMD_NEARBY_DATA = 4;

function getStoredSettings() {
  var settings = {};

  try {
    settings = JSON.parse(localStorage.getItem("clay-settings")) || {};
  } catch (e) {
    console.log("Failed to parse Clay settings: " + e);
  }

  return settings;
}

function getSetting(key) {
  var settings = getStoredSettings();
  if (settings[key] !== undefined && settings[key] !== null && settings[key] !== "") {
    return settings[key];
  }
  return DEFAULT_SETTINGS[key];
}

function getIntSetting(key, min, max) {
  var value = parseInt(getSetting(key), 10);
  if (isNaN(value)) value = DEFAULT_SETTINGS[key];
  if (value < min) value = min;
  if (value > max) value = max;
  return value;
}

function getBackendUrl() {
  var url = getSetting("backend_url");
  return url.replace(/\/+$/, "");
}

function sendQueue(queue, index) {
  if (index >= queue.length) return;

  Pebble.sendAppMessage(
    queue[index],
    function () {
      sendQueue(queue, index + 1);
    },
    function () {
      console.log("Failed to send message " + index);
      Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Send failed" }, null, null);
    }
  );
}

function fetchNearby() {
  Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Getting location..." }, null, null);

  navigator.geolocation.getCurrentPosition(
    function (pos) {
      var lat = pos.coords.latitude;
      var lon = pos.coords.longitude;
      var url =
        getBackendUrl() +
        "/api/v1/stops/nearby?lat=" +
        lat +
        "&lon=" +
        lon +
        "&limit=" +
        getIntSetting("nearby_limit", 1, 20);
      var req = new XMLHttpRequest();

      req.onload = function () {
        if (req.status === 200) {
          var stops = JSON.parse(req.responseText);
          var queue = [];

          queue.push({
            Command: CMD_NEARBY_DATA,
            Count: stops.length
          });

          for (var i = 0; i < stops.length; i++) {
            queue.push({
              Command: CMD_NEARBY_DATA,
              Index: i,
              StopId: stops[i].id,
              StopName: stops[i].name,
              Distance: stops[i].distance_m
            });
          }

          queue.push({ Command: CMD_STATUS, Status: "OK" });
          sendQueue(queue, 0);
        } else {
          Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Error " + req.status }, null, null);
        }
      };

      req.onerror = function () {
        Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Network error" }, null, null);
      };

      req.open("GET", url);
      req.send();
    },
    function (err) {
      console.log("Location error: " + err.message);
      Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Location error" }, null, null);
    },
    { timeout: 15000, maximumAge: 60000 }
  );
}

function fetchDepartures(stopId) {
  Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Loading..." }, null, null);

  var url =
    getBackendUrl() +
    "/api/v1/stops/" +
    encodeURIComponent(stopId) +
    "/departures?limit=" +
    getIntSetting("departure_limit", 1, 3);
  var req = new XMLHttpRequest();

  req.onload = function () {
    if (req.status === 200) {
      var data = JSON.parse(req.responseText);
      var departures = data.departures;
      var queue = [];

      queue.push({
        Command: CMD_DEPARTURE_DATA,
        StopName: data.stop.name,
        Count: departures.length
      });

      for (var i = 0; i < departures.length; i++) {
        queue.push({
          Command: CMD_DEPARTURE_DATA,
          Index: i,
          Mode: departures[i].mode,
          Route: departures[i].route_short_name,
          Headsign: departures[i].headsign,
          Minutes: departures[i].minutes
        });
      }

      queue.push({ Command: CMD_STATUS, Status: "OK" });
      sendQueue(queue, 0);
    } else {
      Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Error " + req.status }, null, null);
    }
  };

  req.onerror = function () {
    Pebble.sendAppMessage({ Command: CMD_STATUS, Status: "Network error" }, null, null);
  };

  req.open("GET", url);
  req.send();
}

Pebble.addEventListener("ready", function () {
  console.log("Pest Stop pkjs ready");
  fetchNearby();
});

Pebble.addEventListener("showConfiguration", function () {
  Pebble.openURL(clay.generateUrl());
});

Pebble.addEventListener("webviewclosed", function (e) {
  if (!e || !e.response || e.response === "CANCELLED") {
    return;
  }

  try {
    clay.getSettings(e.response, false);
    fetchNearby();
  } catch (err) {
    console.log("Failed to save config: " + err);
  }
});

Pebble.addEventListener("appmessage", function (e) {
  var payload = e.payload;
  if (payload.Command === CMD_REQUEST_NEARBY) {
    fetchNearby();
  } else if (payload.Command === CMD_REQUEST_DEPARTURES) {
    fetchDepartures(payload.StopId);
  }
});
