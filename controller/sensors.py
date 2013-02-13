class Sensor:
    def __init__(self, rules, name:str, addr:(str,int)):
        super().__init__()

        # Location to publisher so that the network knows where to subscribe us.
        self.addr = addr

        # The name of this sensor, used to identify it uniquly in messages.
        self.name = name

        # The rules to trigger on events or updates.
        self.rules = rules

class Kinect(Sensor):
    class User:
        def __init__(self, uid):
            self.uid = uid
            self.rawPosition = [0,0,0]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.users = {}

    def reset_state(self, users):
        # Reformat the raw json users list into Users.
        self.users = {}
        for user in users:
            self.users.append(User(user['id'], user['position']))

    def handle_sensor_message(self, json):
        if 'type' not in json:
            print("Dropping invalid message: no type")
            return

        msgType = json['type']

        if msgType == 'HELLO':
            print("Got HELLO from {}".format(self.name))
            if 'users' in json:
                self.reset_state(json['users'])
            return {}
        elif msgType == 'MAYBEADDUSER':
            if 'uid' not in json:
                print("Missing UID in ADDUSER message")
                return
            uid = int(json['uid'])
            self.rules.send_sensor_event(self, 'MAYBEADDUSER')
            if uid in self.users:
                print("Received MAYBEADDUSER for duplicate uid {}: skipping".format(uid))
                return
            print("Kinect {}: MAYBEADDUSER {}".format(self.name, uid))
            self.users[uid] = Kinect.User(uid)
            return {}

        elif msgType == 'ADDUSER':
            if 'uid' not in json:
                print("Missing UID in ADDUSER message")
                return
            uid = int(json['uid'])
            self.rules.send_sensor_event(self, 'ADDUSER')
            if uid in self.users:
                print("Received ADDUSER for duplicate uid {}: skipping".format(uid))
                return
            print("Kinect {}: ADDUSER {}".format(self.name, uid))
            self.users[uid] = Kinect.User(uid)
            return {}

        elif msgType == 'REMOVEUSER':
            if 'uid' not in json:
                print("Missing UID in REMOVEUSER message")
                return
            uid = int(json['uid'])
            if uid not in self.users:
                print("Received ADDUSER for duplicate uid {}: skipping".format(uid))
                return
            print("Kinect {}: REMOVEUSER {}".format(self.name, uid))
            del self.users[uid]
            self.rules.send_sensor_event(self, 'REMOVEUSER')
            return {}

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
            if uid not in self.users:
                self.users[uid] = Kinect.User(uid)
                self.rules.send_sensor_event(self, 'ADDUSER')
            self.users[uid].rawPosition = pos
            #print("Kinect {}: raw POSITION uid: {} @ {}".format(self.name, uid, pos))
            self.rules.send_sensor_event(self, 'POSITION', pos)

        else:
            print("Got unhandled message type: {}".format(msgType))
        return {}

