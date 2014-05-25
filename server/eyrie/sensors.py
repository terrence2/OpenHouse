# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from apscheduler.scheduler import Scheduler

import logging

from mcp.abode import Abode, Area
from mcp.environment import Environment
from mcp.network import Bus as NetworkBus
from mcp.devices import DeviceSet
from mcp.sensors.listener import Listener, ListenerEvent
from mcp.sensors.nerve import Nerve, NerveEvent

log = logging.getLogger("sensors")


# Unfortunately, apscheduler doesn't really handle closures. Instead we have
# to communicate on the global: ugly and fragile.
# Dataflow Requirements:
#   * build_sensors must be called before apschduler is started.
#   * build_sensors must inject the two globals we require.
#   * build_sensors must add a job for this functions.
#   * When apscheduler starts, it will call this and expect the globals to be present.
environment_for_update_ = None
abode_for_update_ = None
def update_environment_on_abode():
    assert environment_for_update_ is not None
    assert abode_for_update_ is not None
    abode_for_update_.set('sunrise_twilight', environment_for_update_.sunrise_twilight)
    abode_for_update_.set('sunrise', environment_for_update_.sunrise)
    abode_for_update_.set('sunset', environment_for_update_.sunset)
    abode_for_update_.set('sunset_twilight', environment_for_update_.sunset_twilight)


def build_sensors(abode: Abode, environment: Environment, network: NetworkBus, scheduler: Scheduler) -> DeviceSet:
    sensors = DeviceSet()

    # Nerves.
    for unique in ('bedroom-north', 'office-north', 'livingroom-south'):
        name = 'nerve-{}'.format(unique)
        log.info("Building nerve: {}".format(name))
        nerve = sensors.add(Nerve(name, (name, NetworkBus.DefaultSensorPort)))
        path = '/eyrie/{}'.format(nerve.room_name)
        room = abode.lookup(path)

        # Put the properties on the abode with a default unset value so that we can know about them.
        room.set('temperature', 'unset')
        room.set('humidity', 'unset')
        room.set('motion', 'unset')

        # Forward updates to the sensor to the abode properties we just attached.
        def make_property_forwarder(bound_room: Area, bound_property_name: str):
            def handler(event: NerveEvent):
                log.info("{}[{}] = {}".format(bound_room.name, bound_property_name, event.value))
                bound_room.set(bound_property_name, event.value)
            return handler
        nerve.listen_temperature(make_property_forwarder(room, 'temperature'))
        nerve.listen_humidity(make_property_forwarder(room, 'humidity'))
        nerve.listen_motion(make_property_forwarder(room, 'motion'))

        # Put on the network.
        network.add_sensor(nerve.remote)

    # Listeners.
    abode.set('control', 'auto:daytime')
    for (room_name, machine) in [('bedroom', 'lemur')]:
        name = 'listener-{}-{}'.format(room_name, machine)
        log.info("Building listener: {}".format(name))
        listener = sensors.add(Listener(name, (machine, NetworkBus.DefaultSensorPort)))
        assert listener.room_name == room_name

        # Forward the commands to the control property.
        def property_forwarder(event: ListenerEvent):
            log.info("/eyrie[control] = {}".format(event.command))
            abode.set('control', event.command)
        listener.listen_for_commands(property_forwarder)

        # Put on the network.
        network.add_sensor(listener.remote)

    # Environment.
    # See comment above update_environment_on_abode for details on why this is insane.
    global abode_for_update_
    global environment_for_update_
    abode_for_update_ = abode
    environment_for_update_ = environment
    update_environment_on_abode()
    scheduler.add_interval_job(update_environment_on_abode, hours=12)

    return sensors
