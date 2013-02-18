from collections import defaultdict, deque
from datetime import datetime, timedelta
from sensors import Sensor
from servos import Servo
import itertools
import numpy

def registration_to_matrix(nums:[float]):
    A = numpy.array(nums, dtype=float, order='C')
    return A.reshape((4,4))

def vec4(x, y, z, w=1):
    return numpy.array((x, y, z, w), dtype=float, order='C')

class Portal:
    """
    A transition area between two rooms.
    """
    def __init__(self, target, width, height, x, y):
        self.target = target
        self.width = width
        self.height = height
        self.x = x
        self.y = y

class Zone:
    """
    A sub-region within a room. This is a convenience structure to help rules
    associate useful lighting states to task areas.
    """
    def __init__(self, name, low_coord, high_coord):
        self.name = name
        self.low = low_coord
        self.high = high_coord

    def contains(self, pos):
        for i in range(3):
            if pos[i] < self.low[i] or pos[i] > self.high[i]:
                return False
        return True

class Room:
    """
    An axis-aligned rectangular extent in a floorplan.
    """
    def __init__(self, name, dimensions):
        super().__init__()
        self.name = name
        self.dimensions = dimensions

        # Special subdivisions within this space.
        self.portals = []
        self.zones = {}

        # The set of devices that observes and affects this room.
        self.sensors = {}
        self.servos = {}

    def add_portal_to(self, other, size, position):
        p = Portal(other, size[0], size[1], position[0], position[1])
        self.portals.append(p)
        return p

    def add_zone(self, zone:Zone):
        self.zones[zone.name] = zone

    def get_zones_at(self, position) -> set:
        out = set()
        for name, zone in self.zones.items():
            if zone.contains(position):
                out.add(zone)
        return out

    def add_servo(self, servo:Servo):
        assert servo.name not in self.servos
        self.servos[servo.name] = servo

    def add_sensor(self, sensor:Sensor, position:(float, float), registration:[float]):
        assert sensor.name not in self.sensors
        self.sensors[sensor.name] = {'position': position,
                                     'matrix': registration_to_matrix(registration),
                                     'sensor': sensor}

    def map_sensor_position_to_room_position(self, sensor:Sensor, position:(float, float, float)):
        roomPos = self.sensors[sensor.name]['matrix'].dot(vec4(*position))
        inside = True
        for i in range(3):
            if roomPos[i] < 0 or roomPos[i] > self.dimensions[i]:
                inside = False
        #print("MAP: {} -> {} in {} => {}".format(position, roomPos, self.dimensions, inside))
        if not inside:
            return None
        return roomPos

class User:
    """
    Our model of a user can bridge multiple kinects, does not randomly
    disappear, and tries not to split in two at random, or appear for no reason
    in the middle of a room.
    """
    class Track:
        """
        This class defines the User's tracking information for a single sensor.
        """

        HISTORY_LENGTH = 0.5 #sec

        def __init__(self, floorplan, sensorUser):
            self.floorplan = floorplan

            # The controlling sensor's tracking info.
            self.sensorUser = sensorUser

            # The state will be marked defunct if we are sensed but cannot be localized to a room.
            self.defunct = True

            # The room the user is currently contained in.
            self.room = None

            # The position of the user's head in room coordinates.
            self.position = None

            # Previous tracked positions: for computing velocity.
            self.history = None

            # The estimated track of the user's head.
            self.velocity = None

            # Try to find out where we are.
            self.initialize_from_sensor()

        def initialize_from_sensor(self):
            room, roomPos = self.find_correct_room()
            if room is None:
                self.defunct = True
                self.room = None
                self.position = None
                self.history = None
                self.velocity = None
                return

            self.defunct = False
            self.room = room
            self.position = roomPos
            self.history = deque([(datetime.now(), roomPos)])
            self.velocity = vec4(0, 0, 0)

        def find_correct_room(self):
            for room in self.floorplan.rooms_with_sensor(self.sensorUser.sensor):
                roomPos = room.map_sensor_position_to_room_position(self.sensorUser.sensor,
                                                                    self.sensorUser.rawPosition)
                if roomPos is not None:
                    return room, roomPos
            return None, None

        def update(self):
            """Update the track based on changed data in the sensor."""
            if self.defunct:
                self.initialize_from_sensor()
                return

            roomPos = self.room.map_sensor_position_to_room_position(self.sensorUser.sensor,
                                                                     self.sensorUser.rawPosition)
            if roomPos is None:
                self.initialize_from_sensor()
                return

            self.position = roomPos

            # Record the new position into history
            now = datetime.now()
            self.history.append((now, self.position))

            # Cull all history older than HISTORY_LENGTH.
            starttime = now - timedelta(seconds=self.HISTORY_LENGTH)
            while self.history[0][0] < starttime:
                self.history.popleft()

            # Estimate velocity from history.
            if len(self.history) > 1:
                starttime, start = self.history[0]
                endtime, end = self.history[-1]
                dt = endtime - starttime
                assert dt > timedelta(0)
                dtsec = dt.seconds + dt.microseconds / 1000000
                self.velocity = (end - start) / dtsec

        def __str__(self):
            if self.defunct:
                return "DEFUNCT"
            return "in {} @ {} v {}({} samp)".format(self.room.name, self.position, self.velocity, len(self.history))


    def __init__(self, floorplan, modelUID, sensorUser):
        self.floorplan = floorplan

        # The model's user id.
        self.uid = modelUID

        # The collection of all sensor users that provide data to this user.
        self.tracks = {sensorUser.key(): self.Track(floorplan, sensorUser)}

        # Any zones the user is currently in.
        self.zones = []

    def remove_user(self, sensorUser):
        """
        Called by the floorplan when one of our observing sensors loses our track.
        """
        del self.tracks[sensorUser.key()]

    def nondefunct(self):
        return [t for _, t in self.tracks.items() if not t.defunct]

    def get_position(self):
        allpos = [t.position for t in self.nondefunct()]
        if not allpos:
            return vec4(-1, -1, -1)
        return sum(allpos) / len(allpos)

    def get_room(self):
        rooms = [t.room for t in self.nondefunct()]
        if not len(rooms):
            return None
        # Ideally this would be from the most recently updated, but that would
        # just lead to instability: just pick one and go with it.
        return rooms[0]

    def update(self, sensorUser):
        """
        Called when one of our tracked positions changes.
        """
        room = self.get_room()
        priorZones = None if not room else room.get_zones_at(self.get_position())

        track = self.tracks[sensorUser.key()]
        track.update()
        # FIXME: Check for and remove diverged tracks

        # Check for enter and leave zone events.
        if room:
            currentZones = room.get_zones_at(self.get_position())
            leftZones = priorZones - currentZones
            enteredZones = currentZones - priorZones
            for zone in leftZones:
                self.floorplan.rules.send_user_event(self, 'LEAVEZONE', zone)
            for zone in enteredZones:
                self.floorplan.rules.send_user_event(self, 'ENTERZONE', zone)

    def __str__(self):
        tracks = ["{}-track{} {}".format(n,i,str(t)) for (n,i),t in self.tracks.items()]
        return "UID{}: {}".format(self.uid, '; '.join(tracks))

class FloorPlan:
    """
    Contains Rooms filled with Sensors and Servos and links them together into a
    conceptual space.

    It is important to have a model separate from the kinect's idea of a user.
    While the kinect is great at detecting people-like-things in depth planes,
    it has no concept at all of time or space. This results in, for example, the
    user splitting in two when walking behind a chair. The kinect thinks like
    this: I see a "human" and a non-human, lets say a "chair" -> but "chair" is
    very close to the human now -> things close to humans are usually just the
    human acting strangly -> lets treat the "chair" as part of the human for the
    moment -> that "chair" is /clearly/ the humans leg bent a bit funny -> that
    chair is a "human" thing -> whoa, another human just walked out from behind
    the human we were tracking -> quick, add another user to the database.
    """
    def __init__(self, name):
        super().__init__()
        self.name = name

        # The rules for us to dispatch events to.
        # Note: this is set by the rules constructor, since we construct the floorplan first.
        self.rules = None

        # A map of the house.
        self.rooms = {}

        # Every sensor and servo in the house.
        self.sensors = {}
        self.servos = {}

        # Every user that we are currently tracking.
        self.users = {}
        self.nextUID = itertools.count(1)

        # Maps sensor names to the rooms they observe: this lets us dispatch new
        # events to the right rooms quickly.
        self.sensorToRooms = defaultdict(list) # {str: [str]}

    def add_room(self, name, width, length, height) -> Room:
        assert name not in self.rooms
        self.rooms[name] = Room(name, (width, length, height))
        return self.rooms[name]

    def get_room(self, name:str) -> Room:
        return self.rooms[name]

    def add_servo(self, servo:Servo, roomName:str):
        if servo.name not in self.servos:
            self.servos[servo.name] = servo
        assert servo is self.servos[servo.name]
        self.rooms[roomName].add_servo(servo)

    def get_servo(self, name:str):
        return self.servos[name]

    def all_servos(self):
        return self.servos.values()

    def add_sensor(self, sensor:Sensor, roomName:str, position:(float,float), registration:[float]):
        if sensor.name not in self.sensors:
            self.sensors[sensor.name] = sensor
        assert sensor is self.sensors[sensor.name]
        sensor.floorplan = self
        self.rooms[roomName].add_sensor(sensor, position, registration)
        self.sensorToRooms[sensor.name].append(roomName)

    def get_sensor(self, name:str):
        return self.sensors[name]

    def all_sensors(self):
        return self.sensors.values()

    def rooms_with_sensor(self, sensor:Sensor) -> [Room]:
        return [self.rooms[name] for name in self.sensorToRooms[sensor.name]]

    def handle_sensor_message(self, json):
        """
        Called by the network to dispatch messages to the sensor they belong to.
        """
        if 'name' not in json:
            print("Dropping invalid message: no name")
            return

        name = json['name']
        if name not in self.sensors:
            print("Got control message from unknown sensor: {}".format(name))
            return

        sensor = self.sensors[name]
        sensor.handle_sensor_message(json)

    def send_sensor_event(self, sensor, eventName, sensorUser, *args):
        """
        Called by the sensors after processing a message so that we can update
        our models with the information. This is called /before/ the RuleSet.
        """
        # Filter out users that we have decided are probably not really users.
        # FIXME: check here if the "user" moved and ungarbage.
        if sensorUser.probablyGarbage:
            return

        if eventName in ['MAYBEADDUSER', 'ADDUSER']:
            """
            We want to filter new users to remove splits. E.g. if a new user
            appears magically right next to another user /and/ there was only
            one user in that location in the near past, /and/ the new user is
            absolutely still. Thus, we have to handle this lazily during our
            first position update, because we have no idea where the new users
            is right now.
            """
            return

        elif eventName == 'REMOVEUSER':
            """
            These currently get delivered /extremely/ lazily... like 2 minutes
            after a user is totally gone, so we handle "removal" in software.
            This event is just to manage the User's sensor list.
            """
            # It is possible for a transitory hit to have never been positioned or associated with a user.
            if sensorUser.modelUID is None:
                return
            self.users[sensorUser.modelUID].remove_user(sensorUser)
            return

        elif eventName == 'POSITION':
            """
            Update the user's velocity track and use that to emit zone-entry and
            exit events and room-exit and handoff events.
            """
            # FIXME: associate multiple sensors to a single user.
            # FIXME: filter out bogus users.
            # FIXME: look for users heading toward a portal and either hand off or mark as garbage.
            # FIXME: remove users at all.

            # Create the new User when we have a position estimate.
            if sensorUser.modelUID is None:
                modelUID = next(self.nextUID)
                self.users[modelUID] = User(self, modelUID, sensorUser)
                sensorUser.modelUID = modelUID

            # Update the user info with our new data.
            self.users[sensorUser.modelUID].update(sensorUser)

        #for uid, u in self.users.items():
        #    print(str(u))

