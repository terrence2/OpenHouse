OpenHouse
===

OpenHouse is:
* Entirely locally managed and run -- no broadcasting your every movement across the internet to
  hundreds of servers you don't control.
* Extremely flexible and adaptable -- everything is a communicating daemon, so adding new
  functionality is easy to do without bricking your house.


Planning
===

Button press in the bedroom ->
REST request to oh_button with <value> ->
oh_button modifies /room/bedroom/radio-button/bedroom-lightswitch.eyrie/state to <value> ->
oh_db applies computed formulas that depend on /^/state, which is /room/bedroom/color ->
    /room/bedroom/color becomes <value>
    oh_db applies computed formulas that depend on /^/color => /room/hall/color
        /room/hall/color becomes <value>
        return [/room/hall/color]
    return [/room/hall/color, /room/bedroom/color]
return [/.../state, /room/hall/color, /room/bedroom/color]
oh_db emits subscription events for everything modified in one go
oh_color receives color change on /room/bedroom/color and /room/hall/color
  in one message
oh_color loops over each key, extracting the room name and builds:
     /room/{bedroom,hall}
oh_color loops over each constant color by light kind and emits
     /room/{bedroom,hall}/hue-light/*/color <- <color-from-palette>
     /room/{bedroom,hall}/hue-livingcolor/*/color <- <color-from-palette>
oh_hue gets a single message with all lights that need to change at once

====
Need to implement:
  * computed files (as above) so that we can aggregate changes in as
    few messages as possible, avoiding the second round trip to the
    formula controller and more importantly, letting us batch the subscription
    events bound for oh_color into a single message.
  * glob matching for sets of strings so that oh_color can send hue light
    updates to multiple, specific rooms in one message.
  * win!
