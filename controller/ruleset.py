
class RuleSet:
    def __init__(self, floorplan):
        super().__init__()
        self.floorplan = floorplan

    def send_sensor_event(self, sensor, eventName, *args):
        method = 'event_' + sensor.name + '_' + eventName
        visitor = getattr(self, method, None)
        if visitor:
            visitor(sensor, *args)
