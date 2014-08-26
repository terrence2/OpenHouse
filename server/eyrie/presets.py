# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.actuators import daylight, moonlight
from eyrie.state import EyrieStateMachine

from mcp.devices import DeviceSet
from mcp.state import StateEvent


def bind_preset_states_to_real_world(state: EyrieStateMachine, actuators: DeviceSet):
    def listen_manual_on(_: StateEvent):
        actuators.select('$hue').set(on=True, color=daylight(1), transition_time=2)

    def listen_manual_low(_: StateEvent):
        actuators.select('$hue').set(on=True, color=daylight(0), transition_time=2)

    def listen_manual_off(_: StateEvent):
        actuators.select('$hue').set(on=False, transition_time=2)

    def listen_manual_sleep(_: StateEvent):
        all_lights = actuators.select('$hue')
        br = all_lights.select('@bedroom')
        off_lights = br.select('#bed') + br.select('#ceiling') + br.select('#tree0') + br.select('#tree1')

        off_lights               .set(on=False, color=moonlight(0), transition_time=2)
        (all_lights - off_lights).set(on=True, color=moonlight(0), transition_time=2)

    def listen_manual_read(_: StateEvent):
        all_lights = actuators.select('$hue')
        bed = all_lights.select('#bed')
        bed               .set(on=True, color=daylight(1), transition_time=0.2)
        (all_lights - bed).set(on=True, color=daylight(0), transition_time=2)

    state.listen_enter_state('manual:on', listen_manual_on)
    state.listen_enter_state('manual:low', listen_manual_low)
    state.listen_enter_state('manual:off', listen_manual_off)
    state.listen_enter_state('manual:sleep', listen_manual_sleep)
    state.listen_enter_state('manual:read', listen_manual_read)

