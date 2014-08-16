# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.actuators import daylight, moonlight
from eyrie.state import EyrieStateMachine

from mcp.abode import Abode, AbodeEvent
from mcp.animation import AnimationController, LinearAnimation
from mcp.color import BHS
from mcp.devices import DeviceSet
from mcp.state import StateEvent

import logging

log = logging.getLogger('auto')

WakeupFadeTime = 30 * 60  # 30 min
SleepFadeTime = 30 * 60  # 30 min


def _handle_wakeup(actuators: DeviceSet, animation: AnimationController, state: EyrieStateMachine):
    # TODO: make these visually appealing. Just a fade in/out right now for simplicity.
    # TODO: keep the lights on full-bore until we detect motion /outside/ the bedroom.
    # Hook wakeup/bedtime to turn the lights on slowly.
    def on_enter_wakeup(_: StateEvent):
        # Move to the expected sleep state, but also turn on the bedside lamp.
        lights = actuators.select('$hue')
        lights.set(on=True, color=moonlight(0))

        def tick(v: BHS):
            lights.set(color=v, transition_time=2.0)

        def finish():
            state.change_state('auto:daytime')

        animation.animate(LinearAnimation(moonlight(0), daylight(1), WakeupFadeTime, tick, finish))
    state.listen_enter_state('auto:wakeup', on_enter_wakeup)


def _handle_bedtime(actuators: DeviceSet, animation: AnimationController, state: EyrieStateMachine):
    # TODO: This is kinda lame. I think we want to animate to full brightness for the full house,
    # TODO: then have the full house enter twighlight as one.
    def on_enter_bedtime(evt: StateEvent):
        # Check if we are already in the sleep state and skip this if we are.
        if evt.prior_state == 'auto:sleep':
            return

        lights = actuators.select('$hue')
        lights.set(on=True, color=daylight(1))

        def tick(v: BHS):
            lights.set(color=v, transition_time=2.0)

        def finish():
            # This state change should shut off the bedside lamp.
            state.change_state('auto:sleep')

        animation.animate(LinearAnimation(daylight(1), moonlight(0), SleepFadeTime, tick, finish))
    state.listen_enter_state('auto:bedtime', on_enter_bedtime)


def _handle_daytime(abode: Abode, actuators: DeviceSet, state: EyrieStateMachine):
    def on_enter_daytime(_: StateEvent):
        for name in abode.subarea_names():
            humans_present = abode.subarea(name).get('humans_present', False)
            actuators.select('$hue').select('@' + name).set(on=True, color=daylight(int(humans_present)))
    state.listen_enter_state('auto:daytime', on_enter_daytime)

    # TODO: Detect ambient light from sunrise/sunset and adjust accordingly.
    # TODO:     Adjust light levels and color on the fly based on time of day.
    # TODO: Allow us to set an ambient color for the house.
    # TODO: Animate fade out on presence = False.
    # TODO: Tie in the kinect when we have people present.
    def on_presence(event: AbodeEvent):
        if state.current != 'auto:daytime':
            log.debug("skipping motion update -- state is {}, not auto:daytime".format(state.current))
            return
        new_lighting = daylight(int(event.property_value))
        log.debug("motion updating lighting state in {} to {}".format(event.target.name, new_lighting))
        actuators.select('$hue').select('@' + event.target.name).set(on=True, color=new_lighting)

    rooms = ['bedroom', 'livingroom', 'office', 'kitchen', 'utility', 'hall']
    for room_name in rooms:
        abode.lookup('/eyrie/{}'.format(room_name)).listen("humans_present", "propertyChanged", on_presence)


def _handle_sleep(abode: Abode, actuators: DeviceSet, state: EyrieStateMachine):
    def on_enter_sleep(_: StateEvent):
        all_lights = actuators.select('$hue')
        br = all_lights.select('@bedroom')
        off_lights = br.select('#bed') + br.select('#ceiling') + br.select('#tree0') + br.select('#tree1')
        on_lights = all_lights - off_lights

        on_lights.set(on=True, color=moonlight(0))
        off_lights.set(on=False)
    state.listen_enter_state('auto:sleep', on_enter_sleep)

    # TODO: Give us a night light in rooms other than the bedroom.
    def on_presence(event: AbodeEvent):
        if state.current != 'auto:sleep':
            return
        assert event.target.name != 'bedroom'
        new_lighting = moonlight(int(event.property_value))
        actuators.select('$hue').select('@' + event.target.name).set(on=True, color=new_lighting)

    rooms = ['livingroom', 'office', 'kitchen', 'utility', 'hall']
    for room_name in rooms:
        abode.lookup('/eyrie/{}'.format(room_name)).listen("humans_present", "propertyChanged", on_presence)


def bind_abode_to_real_world_obeying_state(abode: Abode, actuators: DeviceSet, animation: AnimationController,
                                           state: EyrieStateMachine):
    # Disable any ongoing animation when we leave a state. At the very least we'll want a different animation.
    def on_leave_state(evt: StateEvent):
        if (evt.prior_state == 'auto:daytime' and
                evt.new_state == 'auto:wakeup' and
                abode.get('user_control') == 'auto:daytime'):
            log.info("ABORT attempting to move from auto:daytime to auto:wakeup under auto control.")
            return False
        elif (evt.prior_state == 'auto:sleep' and
                evt.new_state == 'auto:bedtime' and
                abode.get('user_control') == 'auto:sleep'):
            log.info("ABORT attempting to move from auto:sleep to auto:bedtime under auto control.")
            return False
        animation.cancel_ongoing_animation()
    for name in state.all_states():
        state.listen_exit_state(name, on_leave_state)

    _handle_wakeup(actuators, animation, state)
    _handle_bedtime(actuators, animation, state)
    _handle_daytime(abode, actuators, state)
    _handle_sleep(abode, actuators, state)

