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

    def reset_state(self, users):
        # Reformat the raw json users list into Users.
        self.users = {}
        for user in users:
            uid = int(user['uid'])
            p = user['position']
            position = [float(p[0]), float(p[1]), float(p[2])]
            self.users.append(User(uid, self, position))

    def send_event(self, name, uid, *args, **kwargs):
        """
        Dispatches events to where they need to go. Format is:
        Sensor, EventName, Sensor::User, ExtraArgs
        """
        self.floorplan.send_sensor_event(self, name, self.users[uid], *args, **kwargs)
        self.rules.send_sensor_event(self, name, self.users[uid], *args, **kwargs)

    def handle_sensor_message(self, json):
        if 'type' not in json:
            print("Dropping invalid message: no type")
            return

        msgType = json['type']

        if msgType == 'HELLO':
            print("Got HELLO from {}".format(self.name))
            if 'users' in json:
                self.reset_state(json['users'])
            return

        elif msgType == 'MAYBEADDUSER':
            if 'uid' not in json:
                print("Missing UID in ADDUSER message")
                return
            uid = int(json['uid'])
            if uid in self.users:
                print("Received MAYBEADDUSER for duplicate uid {}: skipping".format(uid))
                return
            print("Kinect {}: MAYBEADDUSER {}".format(self.name, uid))
            self.users[uid] = Kinect.User(uid, self, [0, 0, 0])
            self.send_event('MAYBEADDUSER', uid)
            return

        elif msgType == 'ADDUSER':
            if 'uid' not in json:
                print("Missing UID in ADDUSER message")
                return
            uid = int(json['uid'])
            if uid in self.users:
                print("Received ADDUSER for duplicate uid {}".format(uid))
            print("Kinect {}: ADDUSER {}".format(self.name, uid))
            self.users[uid] = Kinect.User(uid, self, [0, 0, 0])
            self.send_event('ADDUSER', uid)
            return

        elif msgType == 'REMOVEUSER':
            if 'uid' not in json:
                print("Missing UID in REMOVEUSER message")
                return
            uid = int(json['uid'])
            if uid not in self.users:
                print("Received ADDUSER for duplicate uid {}: skipping".format(uid))
                return
            print("Kinect {}: REMOVEUSER {}".format(self.name, uid))
            self.send_event('REMOVEUSER', uid)
            del self.users[uid]
            return

        elif msgType == 'POSITION':
            if 'uid' not in json:
                print("Missing UID in POSITION message")
                return
            uid = int(json['uid'])
            for c in 'XYZ':
                if c not in json:
                    print("Missing {}-coordinate in POSITION message".format(c))
                    return
            pos = (float(json['X']), float(json['Y']), float(json['Z']))

            # We can get to this state if we restart the controller.
            if uid not in self.users:
                self.users[uid] = Kinect.User(uid, self, [0, 0, 0])
                self.send_event('ADDUSER', uid)

            self.users[uid].rawPosition = pos
            self.send_event('POSITION', uid, pos)

        else:
            print("Got unhandled message type: {}".format(msgType))
        return {}

