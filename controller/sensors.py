import logging
log = logging.getLogger('sensor')

class Sensor:
    def __init__(self, floorplan, rules, name:str, addr:(str,int)):
        super().__init__()

        # Location to publisher so that the network knows where to subscribe us.
        self.addr = addr

        # The name of this sensor, used to identify it uniquly in messages.
        self.name = name

        # The owning floorplan.
        self.floorplan = floorplan

        # The rules to trigger on events or updates.
        self.rules = rules

class Kinect(Sensor):
    class User:
        def __init__(self, uid, owner, position):
            # The sensor's id.
            self.uid = uid

            # Carry the owner here so we don't have to pass it separately everywhere.
            self.sensor = owner

            # Most recent tracked position.
            self.rawPosition = position

            # Set when our model decides this "user" is an artifact.
            self.probablyGarbage = False

            # This is the uid of the model that this tracked user corresponds
            # to.
            self.modelUID = None

        def key(self):
            return (self.sensor.name, self.uid)

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.users = {}

    def get_status(self):
        """
        Return JSON with raw status info.
        """
        return {name: u.rawPosition for name, u in self.users.items()}

    def send_event(self, name, uid, *args):
        """
        Dispatches events to where they need to go. Format is:
        Sensor, EventName, Sensor::User, ExtraArgs
        """
        sUser = None if uid not in self.users else self.users[uid]
        self.floorplan.send_sensor_event(self, name, sUser, *args)
        self.rules.send_sensor_event(self, name, sUser, *args)

    def handle_sensor_message(self, json):
        if 'type' not in json:
            log.warning("Dropping invalid message: no type")
            return

        msgType = json['type']
        uid = int(json['uid'])

        if msgType == 'MAYBEADDUSER':
            if uid in self.users:
                log.warning("Received MAYBEADDUSER for duplicate uid {}: skipping".format(uid))
                return
            self.send_event('MAYBEADDUSER', uid, uid)
            return

        elif msgType == 'ADDUSER':
            if uid in self.users:
                log.warning("Received ADDUSER for duplicate uid {}".format(uid))
            self.users[uid] = Kinect.User(uid, self, [0, 0, 0])
            self.send_event('ADDUSER', uid)
            return

        elif msgType == 'REMOVEUSER':
            if uid not in self.users:
                log.warning("Received ADDUSER for duplicate uid {}: skipping".format(uid))
                return
            self.send_event('REMOVEUSER', uid)
            del self.users[uid]
            return

        elif msgType == 'POSITION':
            pos = (float(json['X']), float(json['Y']), float(json['Z']))

            # We can get to this state if we restart the controller.
            if uid not in self.users:
                self.users[uid] = Kinect.User(uid, self, [0, 0, 0])
                self.send_event('ADDUSER', uid)

            self.users[uid].rawPosition = pos
            self.send_event('POSITION', uid, pos)

        else:
            log.warning("Got unhandled message type: {}".format(msgType))
        return {}

