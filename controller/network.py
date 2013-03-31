import logging
import zmq
from select import POLLIN, POLLOUT, POLLHUP, POLLNVAL, POLLERR

log = logging.getLogger('network')

class Network:
    DefaultSensorPort = 31975
    DefaultServoPort = 31978
    DefaultControlPort = 31976
    Interval = 500

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

        # Create the control socket.
        self.ctl = self.ctx.socket(zmq.REP)
        self.ctl.bind("tcp://*:" + str(self.DefaultControlPort))
        self.poller.register(self.ctl, POLLIN)

        #stream = self.ctx.socket(zmq.REP)
        #stream.bind("tcp://*:" + str(StreamPort))
        #self.poller.register(stream, POLLIN)

        #bcast = self.ctx.socket(zmq.PUB)
        #bcast.bind("tcp://*:" + str(BroadcastPort))
        #self.poller.register(bcast)



    def run(self):
        while True:
            ready = self.poller.poll(Network.Interval)
            if not ready:
                self.floorplan.handle_timeout()
                continue

            for (sock, event) in ready:
                # Check sensor sockets.
                if sock in self.sensorSocks:
                    if event == POLLIN:
                        self.floorplan.handle_sensor_message(sock.recv_json())
                    else:
                        log.warning("error on sensor socket")

                # Check control socket.
                elif sock == self.ctl:
                    if event == POLLIN:
                        try:
                            rep, doexit = self.floorplan.handle_control_message(sock.recv_json())
                            sock.send_json(rep)
                            if doexit:
                                return 0
                        except Exception as e:
                            import traceback
                            import sys
                            sock.send_json({'error': str(e), 'traceback': traceback.format_tb(sys.last_traceback)})
                    else:
                        log.warning("error on control socket")

