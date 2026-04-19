var BACKEND_URL = "http://127.0.0.1:3000";

var CMD_REQUEST_DEPARTURES = 0;
var CMD_DEPARTURE_DATA = 1;
var CMD_STATUS = 2;
var CMD_REQUEST_NEARBY = 3;
var CMD_NEARBY_DATA = 4;

function sendQueue(queue, index) {
  if (index >= queue.length) return;

  Pebble.sendAppMessage(queue[index], function () {
    sendQueue(queue, index + 1);
  }, function () {
    console.log("Failed to send message " + index);
    Pebble.sendAppMessage(
      { Command: CMD_STATUS, Status: "Send failed" },
      null,
      null
    );
  });
}

function fetchNearby() {
  Pebble.sendAppMessage(
    { Command: CMD_STATUS, Status: "Getting location..." },
    null,
    null
  );

  navigator.geolocation.getCurrentPosition(function (pos) {
    var lat = pos.coords.latitude;
    var lon = pos.coords.longitude;
    var url = BACKEND_URL + "/api/v1/stops/nearby?lat=" + lat + "&lon=" + lon + "&limit=5";
    var req = new XMLHttpRequest();

    req.onload = function () {
      if (req.status === 200) {
        var stops = JSON.parse(req.responseText);
        var queue = [];

        queue.push({
          Command: CMD_NEARBY_DATA,
          Count: stops.length,
        });

        for (var i = 0; i < stops.length; i++) {
          queue.push({
            Command: CMD_NEARBY_DATA,
            Index: i,
            StopId: stops[i].id,
            StopName: stops[i].name,
            Distance: stops[i].distance_m,
          });
        }

        queue.push({ Command: CMD_STATUS, Status: "OK" });
        sendQueue(queue, 0);
      } else {
        Pebble.sendAppMessage(
          { Command: CMD_STATUS, Status: "Error " + req.status },
          null,
          null
        );
      }
    };

    req.onerror = function () {
      Pebble.sendAppMessage(
        { Command: CMD_STATUS, Status: "Network error" },
        null,
        null
      );
    };

    req.open("GET", url);
    req.send();
  }, function (err) {
    Pebble.sendAppMessage(
      { Command: CMD_STATUS, Status: "Location error" },
      null,
      null
    );
  }, { timeout: 15000, maximumAge: 60000 });
}

function fetchDepartures(stopId) {
  Pebble.sendAppMessage(
    { Command: CMD_STATUS, Status: "Loading..." },
    null,
    null
  );

  var url = BACKEND_URL + "/api/v1/stops/" + stopId + "/departures?limit=3";
  var req = new XMLHttpRequest();

  req.onload = function () {
    if (req.status === 200) {
      var data = JSON.parse(req.responseText);
      var departures = data.departures;
      var queue = [];

      queue.push({
        Command: CMD_DEPARTURE_DATA,
        StopName: data.stop.name,
        Count: departures.length,
      });

      for (var i = 0; i < departures.length; i++) {
        queue.push({
          Command: CMD_DEPARTURE_DATA,
          Index: i,
          Route: departures[i].route_short_name,
          Headsign: departures[i].headsign,
          Minutes: departures[i].minutes,
        });
      }

      queue.push({ Command: CMD_STATUS, Status: "OK" });
      sendQueue(queue, 0);
    } else {
      Pebble.sendAppMessage(
        { Command: CMD_STATUS, Status: "Error " + req.status },
        null,
        null
      );
    }
  };

  req.onerror = function () {
    Pebble.sendAppMessage(
      { Command: CMD_STATUS, Status: "Network error" },
      null,
      null
    );
  };

  req.open("GET", url);
  req.send();
}

Pebble.addEventListener("ready", function () {
  console.log("Pest Stop pkjs ready");
  fetchNearby();
});

Pebble.addEventListener("appmessage", function (e) {
  var payload = e.payload;
  if (payload.Command === CMD_REQUEST_NEARBY) {
    fetchNearby();
  } else if (payload.Command === CMD_REQUEST_DEPARTURES) {
    fetchDepartures(payload.StopId);
  }
});
