# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from collections import namedtuple
from datetime import datetime, timedelta

from mcp.abode import Abode, AbodeEvent, Area
from mcp.cronish import Cronish

log = logging.getLogger('presence')


def _get_cronish_time_from_now(offset: int) -> (set, set, set):
    """
    Compute and return a cron-compatible timeset representing now plus |offset| seconds.
    """
    now = datetime.now()
    when = now + timedelta(seconds=offset)
    result = {when.weekday()}, {when.hour}, {when.minute}
    log.debug("getting cronish offset: {} + {} => {} : {}".format(now, offset, when, result))
    return result


WatchedProperty = namedtuple('WatchedProperty', ('area', 'sensor', 'lifetime'))


def _bind_area_to_presence(cronish: Cronish, area: Area, properties: [WatchedProperty], timeout: int):
    cron_task_name = "{}_presence_update_timeout".format(area.name)

    def _timeout_presence():
        # Sanity check -- if any of the properties here went true, we should have reset the timeout.
        # Races are possible, which is why we only log the error.
        if any((watched_.area.get(watched_.sensor) for watched_ in properties)):
            log.error("setting humans_present to false with a True human sensor")

        # Clear the presence data and ensure we won't run again until
        # we see more evidence of humans.
        area.set('humans_present', False)
        cronish.update_task_time(cron_task_name, set(), set(), set())

    def _check_presence_conditions(_: AbodeEvent):
        if any((watched_.area.get(watched_.sensor) for watched_ in properties)):
            area.set('last_detected_humans', datetime.now())
            area.set('humans_present', True)
            cronish.update_task_time(cron_task_name, *_get_cronish_time_from_now(timeout))

    cronish.register_task(cron_task_name, _timeout_presence)
    cronish.update_task_time(cron_task_name, set(), set(), set())
    for watched in properties:
        watched.area.listen(watched.sensor, 'propertyTouched', _check_presence_conditions)

    # Set initial state.
    area.set('last_detected_humans', 'never')
    area.set('humans_present', False)
    _check_presence_conditions(None)


def bind_abode_to_presence(abode: Abode, cronish: Cronish):
    office = abode.lookup('/eyrie/office')
    bedroom = abode.lookup('/eyrie/bedroom')
    kitchen = abode.lookup('/eyrie/kitchen')
    utility = abode.lookup('/eyrie/utility')
    hall = abode.lookup('/eyrie/hall')
    livingroom = abode.lookup('/eyrie/livingroom')
    presence_sensors = {
        office: [
            WatchedProperty(office, 'wemo_motion_desk', 10),
            WatchedProperty(office, 'wemo_motion_west', 3),
            WatchedProperty(office, 'wemo_motion_east', 3),
        ],
        bedroom: [
            WatchedProperty(bedroom, 'wemo_motion_desk', 10),
            WatchedProperty(bedroom, 'wemo_motion_south', 5),
        ],
        kitchen: [
            WatchedProperty(kitchen, 'wemo_motion_sink', 5),
            WatchedProperty(kitchen, 'wemo_motion_west', 3),
            WatchedProperty(utility, 'wemo_motion_north', 3),
        ],
        utility: [
            WatchedProperty(utility, 'wemo_motion_north', 1),
            WatchedProperty(kitchen, 'wemo_motion_sink', 3),
        ],
        hall: [
            WatchedProperty(office, 'wemo_motion_east', 1),
            WatchedProperty(bedroom, 'wemo_motion_south', 1),
            WatchedProperty(kitchen, 'wemo_motion_west', 1),
            WatchedProperty(livingroom, 'wemo_motion_north', 1),
            #WatchedProperty(bathroom, 'wemo_motion_west'),
        ],
        livingroom: [
            WatchedProperty(livingroom, 'wemo_motion_north', 1),
        ]
    }
    for area, properties in presence_sensors.items():
        _bind_area_to_presence(cronish, area, properties, 5 * 60)

