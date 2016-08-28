OpenHouse
===

OpenHouse is:
* Entirely locally managed and run -- no broadcasting your every movement across the internet to
  hundreds of servers you don't control.
* Extremely flexible and adaptable -- everything is a communicating daemon, so adding new
  functionality is easy to do without bricking your house.

Architecture
===

Devices are handled by.
oh_wemo (WeMo Motion Detector)
  => /room/**/wemo-motion-detector/raw-state == true|false
oh_motion_filter (Hysteresis)
  <= /room/**/wemo-motion-detector/state
  => /root/**/wemo-motion-detector/motion
  
oh_rest (Via-Buttons)
  => /root/**/radio-button/*/raw-state
                  

oh_apply
  <= /root/(name)/radio-button/*/state 
  <= /room/(name)/switch/*/state ???
  <= /room/(name)/*motion-detector/motion
  => /room/(name)/state

oh_hue
  <= /root/**/hue-light/*/color
  => ##controls lights##

