OpenHouse
===

OpenHouse is:
* Entirely locally managed and run -- no broadcasting your every movement across the internet to
  hundreds of servers you don't control.
* Extremely flexible and adaptable -- everything is a communicating daemon, so adding new
  functionality is easy to do without bricking your house.

Architecture
===

oh_wemo (WeMo Motion Detector)
  Listens for WeMo motion and button events and updates:
      => /room/**/wemo-motion-detector/raw-state == true|false
oh_motion_filter (Hysteresis)
  Listens for changes to WeMo motion-detector states and adds
  hysteresis give a better proxy for presence.
      <= /room/**/wemo-motion-detector/state
      => /root/**/wemo-motion-detector/motion
  
oh_button
  Listens for HTTP requests from buttons and maps that to state
  changes in the appropriate path.
  => /root/**/radio-button/**/state
 
oh_formula
  Apply formulas. For example, listening for button states and setting
  a room's color appropriately.

oh_color
  Listen for color changes in a room and to a color and set the *light
  states appropriately.

oh_hue
  <= /root/**/hue-light/*/color
  => ##controls lights##

