# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp.abode import Abode

import logging
import os
import os.path
import subprocess

log = logging.getLogger("database")


def add_data_recorders(abode: Abode, db_path: str):
    def make_recorder(room_name: str, input_name: str) -> callable:
        database_file = os.path.join(db_path, room_name + '-' + input_name + '.rrd')

        def recorder(event):
            assert event.property_name == input_name
            log.debug("Recording {} for {} - {}".format(input_name, room_name, int(event.property_value)))
            subprocess.check_output(["rrdtool", "update", database_file, "--",
                                     "N:{}".format(int(event.property_value))])
        return recorder

    for room in ('bedroom', 'office', 'livingroom'):
        for input_ in ('temperature', 'humidity', 'motion'):
            abode.lookup('/eyrie/' + room).listen(input_, 'propertyTouched', make_recorder(room, input_))
