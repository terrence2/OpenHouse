import logging
log = logging.getLogger('rules')

class RuleSet:
    """
    All sensors feed thier events to the RuleSet. The ruleset constructs a
    function name from the specific sensor and event and tries to call that
    method on itself. Subclasses can override these methods to receive the event.

    Messages:
        event_SensorName_EVENT(self, sensor, *args)
    """
    def __init__(self, floorplan):
        super().__init__()
        self.floorplan = floorplan
        assert self.floorplan.rules is None
        self.floorplan.rules = self

    def send_sensor_event(self, sensor, eventName, *args):
        method = 'event_' + sensor.name + '_' + eventName
        visitor = getattr(self, method, None)
        if visitor:
            visitor(sensor, *args)
        else:
            log.warning("Unhandled Sensor Event: {} -> {}".format(eventName, method))

    def send_user_event(self, user, eventName, *args):
        method = 'user_' + eventName;
        visitor = getattr(self, method, None)
        if visitor:
            visitor(user, *args)
        else:
            log.warning("Unhandled User Event: {} -> {}".format(eventName, method))
