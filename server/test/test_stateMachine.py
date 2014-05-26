# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from unittest import TestCase

from mcp.state import StickyNestedStateMachine, StateEvent


class MyStateMachine(StickyNestedStateMachine):
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
            'sleep'
        }
    }
    StickyState = 'manual'


class TestStateMachine(TestCase):
    def test_new(self):
        machine = MyStateMachine("auto:daytime")
        self.assertEqual(machine.current, "auto:daytime")
        self.assertEqual(machine.current_state.left, "auto")
        self.assertEqual(machine.current_state.right, "daytime")

        daytime = 0
        manualon = 0
        manualoff = 0

        def enter_auto_daytime(event: StateEvent):
            nonlocal daytime
            daytime += 1

        def leave_auto_daytime(event: StateEvent):
            nonlocal daytime
            daytime -= 1

        def enter_manual_on(event: StateEvent):
            nonlocal manualon
            manualon += 1

        def leave_manual_on(event: StateEvent):
            nonlocal manualon
            manualon -= 1

        def enter_manual_off(event: StateEvent):
            nonlocal manualoff
            manualoff += 1

        def leave_manual_off(event: StateEvent):
            nonlocal manualoff
            manualoff -= 1

        machine.listen_enter_state("auto:daytime", enter_auto_daytime)
        machine.listen_exit_state("auto:daytime", leave_auto_daytime)
        machine.listen_enter_state("manual:on", enter_manual_on)
        machine.listen_exit_state("manual:on", leave_manual_on)
        machine.listen_enter_state("manual:off", enter_manual_off)
        machine.listen_exit_state("manual:off", leave_manual_off)

        self.assertTrue(machine.change_state("manual:on"))
        self.assertEqual(daytime, -1)
        self.assertEqual(manualon, 1)
        self.assertEqual(manualoff, 0)

        # Re-entering the same state should not re-trigger.
        self.assertFalse(machine.change_state("manual:on"))
        self.assertEqual(daytime, -1)
        self.assertEqual(manualon, 1)
        self.assertEqual(manualoff, 0)

        # Now that we are in manual, we should be sticky.
        self.assertFalse(machine.change_state("auto:daytime"))
        self.assertEqual(daytime, -1)
        self.assertEqual(manualon, 1)
        self.assertEqual(manualoff, 0)

        # But switching to other manual modes should be okay.
        self.assertTrue(machine.change_state("manual:off"))
        self.assertEqual(daytime, -1)
        self.assertEqual(manualon, 0)
        self.assertEqual(manualoff, 1)

        # And we need user action to switch back.
        self.assertTrue(machine.change_user_state("auto:daytime"))
        self.assertEqual(daytime, 0)
        self.assertEqual(manualon, 0)
        self.assertEqual(manualoff, 0)
