import logging
import zmq
from select import POLLIN
from threading import Thread

from actuators import ZmqActuator
from sensormodel import SensorModel

log = logging.getLogger('network')


class Network(Thread):
    """
    The global message bus for an MCP instance. Receives sensor traffic from
    the network and forwards it to the connected model for further processing.
    """

    DefaultSensorPort = 31975
    DefaultActuatorPort = 31978
    DefaultControlPort = 31976
    Interval = 500

    def __init__(self, floorplan, sensor_model:SensorModel):
        super().__init__(daemon=False)
        self.ready_to_exit = False

        self.floorplan = floorplan
        self.model = sensor_model

        self.ctx = zmq.Context()
        self.poller = zmq.Poller()

        # Subscribe to all sensors.
        self.sensor_socks = []
        for sensor in self.floorplan.all_sensors():
            sock = self.ctx.socket(zmq.SUB)
            address = "tcp://" + sensor.addr[0] + ":" + str(sensor.addr[1])
            log.info("Connecting to sensor at: {}".format(address))
            sock.connect(address)
            sock.setsockopt(zmq.SUBSCRIBE, b'')
            self.sensor_socks.append(sock)
            self.poller.register(sock, POLLIN)

        # Create the update broadcaster and let all actuators know about it.
        self.update_sock = self.ctx.socket(zmq.PUB)
        self.update_sock.bind("tcp://*:" + str(Network.DefaultActuatorPort))
        for actuator in self.floorplan.all_actuators():
            if isinstance(actuator, ZmqActuator):
                actuator.set_socket(self.update_sock)

        # Create the control socket.
        self.ctl = self.ctx.socket(zmq.REP)
        self.ctl.bind("tcp://*:" + str(self.DefaultControlPort))
        self.poller.register(self.ctl, POLLIN)

        # stream = self.ctx.socket(zmq.REP)
        # stream.bind("tcp://*:" + str(StreamPort))
        # self.poller.register(stream, POLLIN)

        # bcast = self.ctx.socket(zmq.PUB)
        # bcast.bind("tcp://*:" + str(BroadcastPort))
        # self.poller.register(bcast)

    def run(self):
        while not self.ready_to_exit:
            ready = self.poller.poll(Network.Interval)
            if not ready:
                self.model.handle_timeout()
                continue

            for (sock, event) in ready:
                self.check_sensors(sock, event)
                self.check_control(sock, event)

    def check_sensors(self, sock, event):
        if sock not in self.sensor_socks:
            return
        if event != POLLIN:
            log.warning("unknown error on sensor socket")
            return
        try:
            self.model.handle_sensor_message(sock.recv_json())
        except KeyError as ex:
            log.exception("failed to process sensor message")

    def check_control(self, sock, event):
        if sock != self.ctl:
            return

        if event == POLLIN:
            try:
                rep, do_exit = self.floorplan.handle_control_message(sock.recv_json())
                sock.send_json(rep)
                if do_exit:
                    return 0
            except Exception as e:
                import traceback
                import sys
                sock.send_json({'error': str(e),
                                'traceback': traceback.format_tb(
                                sys.last_traceback)})
        else:
            log.warning("error on control socket")
