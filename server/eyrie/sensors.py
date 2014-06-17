# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from mcp.abode import Abode, Area
from mcp.cronish import Cronish
from mcp.environment import Environment
from mcp.network import Bus as NetworkBus
from mcp.devices import DeviceSet
from mcp.sensors.listener import Listener, ListenerEvent
from mcp.sensors.nerve import Nerve, NerveEvent

log = logging.getLogger("sensors")


def build_sensors(abode: Abode, environment: Environment, network: NetworkBus, cronish: Cronish) -> DeviceSet:
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
    abode.set('user_control', 'auto:daytime')
    for (room_name, machine) in [('bedroom', 'lemur')]:
        name = 'listener-{}-{}'.format(room_name, machine)
        log.info("Building listener: {}".format(name))
        listener = sensors.add(Listener(name, (machine, NetworkBus.DefaultSensorPort)))
        assert listener.room_name == room_name

        # Forward the commands to the control property.
        def property_forwarder(event: ListenerEvent):
            log.info("/eyrie[user_control] = {}".format(event.command))
            abode.set('user_control', event.command)
        listener.listen_for_commands(property_forwarder)

        # Put on the network.
        network.add_sensor(listener.remote)

    # Environment.
    def update_environment_on_abode():
        abode.set('sunrise_twilight', environment.sunrise_twilight)
        abode.set('sunrise', environment.sunrise)
        abode.set('sunset', environment.sunset)
        abode.set('sunset_twilight', environment.sunset_twilight)
    cronish.register_task('update_environment_on_abode', update_environment_on_abode)
    cronish.update_task_time('update_environment_on_abode',
                             days_of_week={0, 1, 2, 3, 4, 5, 6}, hours={0}, minutes=set())

    return sensors
