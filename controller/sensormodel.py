import logging

log = logging.getLogger('SensorModel')


class SensorModel:
    """
    The sensor model receives input from the suite of sensors configured by the
    floorplan and post-processes them to account for known sensor defects. The
    output is a set of tracked users which is more reliable and can account for
    several overlapping sensors.
    """

    class User:

        def __init__(self, owner):
            # All sources of input that currently feed into this User.
            self.inputs = set()  # (sensor, trackid)

    def __init__(self, floorplan):
        self.floorplan = floorplan

        # The floorplan also tracks all sensors, since they are part of the
        # physical layout. We copy the dict local for fast access.
        self.sensors = {}
        for sensor in floorplan.all_sensors():
            self.sensors[sensor.name] = sensor

    def think(self):
        """
        After we process new messages, update our model and dispatch any new
        events to the user model.
        """
        for name, sensor in self.sensors.items():
            pass

    def handle_timeout(self):
        """
        Network.Interval expired with no sensor input.
        """

    def handle_sensor_message(self, json):
        """
        Receive a raw sensor message from the network.
        """
        sensor = self.sensors[json['name']]
        sensor.handle_sensor_message(json)

