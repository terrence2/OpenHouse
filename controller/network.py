import zmq
from select import POLLIN, POLLOUT, POLLHUP, POLLNVAL, POLLERR

class Network:
    DefaultSensorPort = 31975
    DefaultServoPort = 31978

    def __init__(self, floorplan):
        self.floorplan = floorplan

        self.ctx = zmq.Context()
        self.poller = zmq.Poller()

        # Subscribe to all sensors.
        self.sensorSocks = []
        for sensor in self.floorplan.all_sensors():
            sock = self.ctx.socket(zmq.SUB)
            sock.connect("tcp://" + sensor.addr[0] + ":" + str(sensor.addr[1]))
            sock.setsockopt(zmq.SUBSCRIBE, b'')
            self.sensorSocks.append(sock)
            self.poller.register(sock, POLLIN)

        # Create the update broadcaster and let all servos know about it.
        self.updateSock = self.ctx.socket(zmq.PUB)
        self.updateSock.bind("tcp://*:" + str(Network.DefaultServoPort))
        for servo in self.floorplan.all_servos():
            servo.set_socket(self.updateSock)

        #self.sensorctl = self.ctx.socket(zmq.REP)
        #self.sensorctl.bind("tcp://*:" + str(self.SensorControlPort))
        #self.poller.register(self.sensorctl, POLLIN)

        #stream = self.ctx.socket(zmq.REP)
        #stream.bind("tcp://*:" + str(StreamPort))
        #self.poller.register(stream, POLLIN)

        #bcast = self.ctx.socket(zmq.PUB)
        #bcast.bind("tcp://*:" + str(BroadcastPort))
        #self.poller.register(bcast)



    def run(self):
        while True:
            ready = self.poller.poll(2000)
            if not ready:
                continue

            for (sock, event) in ready:
                # Check for specific well-known sockets.
                if sock in self.sensorSocks:
                    if event == POLLIN:
                        # TODO: move name computation to controller.
                        self.floorplan.handle_sensor_message(sock.recv_json())
                    else:
                        print("error on sensor socket")
                        return 1


                """
                # Check servo sockets.
                for s in self.servos:
                    if sock == s:
                        if event == POLLOUT:
                            msg = self.servos[s][0]
                            self.servos[s] = self.servos[s][1:]
                            s.send(msg)
                            if not self.servos[s]:
                                self.poller.unregister(s)

                        else:
                            print("Unexpected event on socket: {}".format(s))
                """
