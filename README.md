OpenHouse
===

OpenHouse is:
* Secure -- no broadcasting your every movement across the internet to
  hundreds of servers you don't control.
* Easy -- IoT interations are represented in Yggdrasil, a dataflow language
  that makes orchestrating complex interactions trivial.
* Fast -- OpenHouse is 100% Rust. Switching on lights should take
  nanoseconds, not milliseconds.

Getting Started
===============
1) `cargo install --git https://github.com/terrence2/OpenHouse.git`
2) `$EDITOR home.ygg`
3) `oh_daemon --config home.ygg`