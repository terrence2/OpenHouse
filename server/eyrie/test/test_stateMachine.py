# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.state import StateMachine, EyrieState
from unittest import TestCase


class TestStateMachine(TestCase):
    def test_current(self):
        n_enter_manual = 0
        n_leave_manual = 0
        n_wakeup = 0
        n_daytime = 0
        n_bedtime = 0
        n_sleep = 0

        def on_enter_manual(event):
            self.assertIs(event.new_state, EyrieState.Manual)
            nonlocal n_enter_manual
            n_enter_manual += 1

        def on_leave_manual(event):
            self.assertIs(event.prior_state, EyrieState.Manual)
            nonlocal n_leave_manual
            n_leave_manual += 1

        def on_wakeup(event):
            self.assertIs(event.new_state, EyrieState.Wakeup)
            nonlocal n_wakeup
            n_wakeup += 1

        def on_daytime(event):
            self.assertIs(event.new_state, EyrieState.Daytime)
            nonlocal n_daytime
            n_daytime += 1

        def on_bedtime(event):
            self.assertIs(event.new_state, EyrieState.Bedtime)
            nonlocal n_bedtime
            n_bedtime += 1

        def on_sleep(event):
            self.assertIs(event.new_state, EyrieState.Sleep)
            nonlocal n_sleep
            n_sleep += 1

        state = StateMachine()
        self.assertEqual(state.current, EyrieState.Manual)

        state.listen_enter_manual(on_enter_manual)
        state.listen_leave_manual(on_leave_manual)
        state.listen_wakeup(on_wakeup)
        state.listen_daytime(on_daytime)
        state.listen_bedtime(on_bedtime)
        state.listen_sleep(on_sleep)

        state.leave_manual(EyrieState.Wakeup)
        self.assertEqual(state.current, EyrieState.Wakeup)
        self.assertEqual(n_leave_manual, 1)
        self.assertEqual(n_wakeup, 1)

        state.daytime()
        self.assertEqual(state.current, EyrieState.Daytime)
        self.assertEqual(n_daytime, 1)

        state.bedtime()
        self.assertEqual(state.current, EyrieState.Bedtime)
        self.assertEqual(n_bedtime, 1)

        state.sleep()
        self.assertEqual(state.current, EyrieState.Sleep)
        self.assertEqual(n_sleep, 1)

        # Don't re-fire on same transition.
        state.sleep()
        self.assertEqual(state.current, EyrieState.Sleep)
        self.assertEqual(n_sleep, 1)

        # Don't add multi-callbacks.
        state.listen_wakeup(on_wakeup)
        state.listen_wakeup(on_wakeup)
        state.listen_wakeup(on_wakeup)
        state.listen_wakeup(on_wakeup)
        state.listen_wakeup(on_wakeup)
        state.wakeup()
        self.assertEqual(state.current, EyrieState.Wakeup)
        self.assertEqual(n_wakeup, 2)

        # Manual-mode is sticky.
        state.enter_manual()
        self.assertEqual(state.current, EyrieState.Manual)
        self.assertEqual(n_enter_manual, 1)

        state.wakeup()
        self.assertEqual(state.current, EyrieState.Manual)
        self.assertEqual(n_wakeup, 2)

        state.daytime()
        self.assertEqual(state.current, EyrieState.Manual)
        self.assertEqual(n_daytime, 1)
