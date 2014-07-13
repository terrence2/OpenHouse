# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

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


def _bind_office_to_presence(office: Area, cronish: Cronish):
    def _timeout_presence():
        # Assert sanity.
        desk = office.get('wemo_motion_desk')
        #west = office.get('wemo_motion_west')
        #east = office.get('wemo_motion_east')
        west = east = False
        assert not desk and not west and not east, "Got timeout with "

        # Clear the presence data and ensure we won't run again until
        # we see movement again.
        office.set('humans_present', 0)
        cronish.update_task_time('office_presence_update', set(), set(), set())

    def _check_presence_conditions(_: AbodeEvent):
        desk = office.get('wemo_motion_desk')
        #west = office.get('wemo_motion_west')
        #east = office.get('wemo_motion_east')
        west = east = False
        if desk or west or east:
            office.set('last_detected_motion', datetime.now())
            cronish.update_task_time('office_presence_update', *_get_cronish_time_from_now(5 * 60))
            office.set('humans_present', 1)

    cronish.register_task('office_presence_update', _timeout_presence)
    cronish.update_task_time('office_presence_update', set(), set(), set())
    office.listen('wemo_motion_desk', 'propertyTouched', _check_presence_conditions)
    #office.listen('wemo_motion_west', 'propertyTouched', _check_presence_conditions)
    #office.listen('wemo_motion_east', 'propertyTouched', _check_presence_conditions)


def bind_abode_to_presence(abode: Abode, cronish: Cronish):
    _bind_office_to_presence(abode.lookup('/eyrie/office'), cronish)
