# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp.state import StickyNestedStateMachine


class EyrieStateMachine(StickyNestedStateMachine):
    States = {
        'auto': {
            'wakeup',
            'daytime',
            'bedtime',
            'sleep'
        },
        'manual': {
            'on',
            'low',
            'off',
            'sleep',
            'read',
            'unset',
        }
    }
    StickyState = 'manual'


