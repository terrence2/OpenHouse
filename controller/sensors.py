# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging
import subprocess

from collections import deque
from datetime import datetime, timedelta

from lib import vec4
from filesystem import File, Dir

log = logging.getLogger('sensor')


class Sensor(Dir):
    def __init__(self, floorplan, name, addr):
        super().__init__(floorplan)

        # Location to publisher so that the network knows where to subscribe
        # us.
        self.addr = addr

        # The name of this sensor, used to identify it uniquly in messages.
        self.name = name


class Nerve(Sensor):
    DatabaseLocation = "/storage/raid/data/var/db/mcp/{}.rrd"

    def __init__(self, floorplan, name: str, addr: str):
        super().__init__(floorplan, name, addr)
        self.database = self.DatabaseLocation.format(self.name)

        self.last_temperature = None
        self.last_humidity = None
        self.last_motion_state = False

        self._fs_celsius = File(self.read_celsius, None)
        self._fs_fahrenheit = File(self.read_fahrenheit, None)
        self._fs_humidity = File(self.read_humidity, None)
        self._fs_motion = File(self.read_motion, None)

    def read_celsius(self) -> str:
        if self.last_temperature is None:
            return "Waiting for reading\n"
        return str(self.last_temperature) + "\n"

    def read_fahrenheit(self) -> str:
        if self.last_temperature is None:
            return "Waiting for reading\n"
        return str(self.last_temperature * 9.0 / 5.0 + 32.0) + "\n"

    def read_humidity(self) -> str:
        if self.last_humidity is None:
            return "Waiting for reading\n"
        return str(self.last_humidity) + "\n"

    def read_motion(self) -> str:
        return str(self.last_motion_state) + "\n"

    def handle_sensor_message(self, json):
        """
        Called by the sensor model to inform us of new messages from the
        network.
        """
        msg_type = json['type']
        if msg_type == 'TEMP_HUMIDITY':
            self.last_temperature = float(json['temp'])
            self.last_humidity = float(json['humidity'])
            log.debug("Nerve TEMP_HUMIDITY: {} {}".format(self.last_temperature, self.last_humidity))
            subprocess.check_output(["rrdtool", "update", self.database, "--",
                                     "N:{}:{}".format(self.last_temperature, self.last_humidity)])
        elif msg_type == 'MOVEMENT':
            self.last_motion_state = bool(json['state'])
            log.debug("Movement state: {}".format(self.last_motion_state))
        else:
            log.error("Unrecognized message type from Nerve {}: {}".format(self.name, msg_type))


class Kinect(Sensor):
    class Track:
        class State:
            # Kinect maybe sees someone.
            PENDING = 0
            # Kinect has identified a person, but hasn't given us position yet.
            DISCOVERED = 1
            # An active uid.
            TRACKING = 2
            # A uid which is probably garbage.
            ARTIFACT = 3

        # The number of history items. We keep this in terms of number of
        # points rather than time for performance.
        HistorySize = 128

        # The number of seconds a track must be stuck at a single position for
        # the track to be considered an artifact.
        NotRealPersonTimeout = timedelta(seconds=2)

        class Pos:
            def __init__(self, t, pos):
                self.x = pos[0]
                self.y = pos[1]
                self.z = pos[2]
                self.t = t

        def __init__(self, uid, owner):
            # The sensor's assigned id for this "user".
            self.uid = uid

            # The current state of this "user". Depending on the state, some
            # fields of this structure may not be valid.
            self.state = Kinect.Track.State.PENDING

            # Carry the owner here so we don't have to pass it separately
            # everywhere.
            self.sensor = owner

            # Most recent tracked position.
            self.rawposition_ = deque([], self.HistorySize)
            self.positions_ = {}
            for name in self.sensor.transforms:
                self.positions_[name] = deque([], self.HistorySize)

        def add_position(self, pos):
            t = datetime.utcnow()

            # If this is our first point, switch us to TRACKING.
            if len(self.rawposition_) == 0:
                self.state = Kinect.Track.State.TRACKING

            # Deque automatically drops from the opposite end.
            self.rawposition_.appendleft(Kinect.Track.Pos(t, pos))
            for name, (pos, matrix) in self.sensor.transforms.items():
                xformed = Kinect.Track.Pos(t, matrix.dot(vec4(*pos)))
                self.positions_[name].appendleft(xformed)

            # If the history is 100% stable for a full 2 seconds, move state
            # to ARTIFACT.
            if self.is_dead_for(self.NotRealPersonTimeout):
                self.state = Kinect.Track.State.ARTIFACT

        def is_dead_for(self, delta):
            if len(self.rawposition_) == 0:
                return False
            initial = self.rawposition_[0]
            tracked_time = self.rawposition_[-1].t - initial.t

            # Must have delta seconds of history.
            if tracked_time < delta:
                return False

            # If any point in the first delta time is different, the track is
            # not considered dead.
            for pos in self.rawposition_:
                if pos.t - initial.t > delta:
                    return True
                if not pos.is_same_as(initial):
                    return False
            return True

        def key(self):
            return (self.sensor.name, self.uid)

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        # Raw kinect identified tracks.
        self.tracks = {}

        # Transformations we want to apply to each user.
        self.transforms = {}

    def get_status(self):
        """
        Return JSON with raw status info.
        """
        return {name: u.rawPosition_ for name, u in self.tracks.items()}

    def add_registration(self, name, pos, matrix):
        self.transforms[name] = (pos, matrix)

    def on_maybe_add_user(self, uid):
        """
        Called when the kinect has identified what may be a person. The
        middleware will devote some extra processing to figuring out if this is
        actually a person and give an ADDUSER or a REMOVEUSER depending on the
        results.
        """
        if uid in self.tracks:
            raise KeyError("duplicate uid '{}' on '{}'".format(uid, self.name))
        self.tracks[uid] = Kinect.Track(uid, self)

    def on_add_user(self, uid):
        """
        Called when the kinect has decided the new thing is a person.
        """
        self.tracks[uid].state = Kinect.Track.State.DISCOVERED

    def on_remove_user(self, uid):
        """
        Called when the kinect has given up on a track and decided to drop it.
        This will generally happen LONG after the user has walked out of the
        room.
        """
        if uid in self.tracks:
            raise KeyError("got remove for unknown uid '{}' on '{}'"
                           .format(uid, self.name))
        del self.tracks[uid]

    def on_position(self, uid, pos):
        # We can get to this state if we restart the controller.
        if uid not in self.tracks:
            self.tracks[uid] = Kinect.Track(uid, self)
            self.tracks[uid].state = Kinect.Track.State.DISCOVERED

        self.tracks[uid].add_position(pos)

    def handle_sensor_message(self, json):
        """
        Called by the sensor model to inform us of new messages from the
        network.
        """
        msgType = json['type']
        uid = int(json['uid'])

        if msgType == 'MAYBEADDUSER':
            self.on_maybe_add_user(uid)

        elif msgType == 'ADDUSER':
            self.on_add_user(uid)

        elif msgType == 'REMOVEUSER':
            self.on_remove_user(uid)

        elif msgType == 'POSITION':
            pos = (float(json['X']), float(json['Y']), float(json['Z']))
            self.on_position(uid, pos)

        else:
            log.warning("Got unhandled message type: {}".format(msgType))

