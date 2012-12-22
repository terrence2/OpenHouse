#!/usr/bin/python3
import argparse
import serial
import socket
import sys
import time
import zmq

DefaultTTY = '/dev/ttyACM0'
DefaultControllerHost = 'gorilla'
DefaultServoPort = 31978

def main():
	parser = argparse.ArgumentParser(description='Control a LED lightstrip attached to an Arduino.')
	parser.add_argument('command', metavar='NAME', type=str, help='The data to send.')
	parser.add_argument('--tty', metavar='TTY', type=str, default=DefaultTTY, help='The TTY the arduino is connected on.')
	args = parser.parse_args()

	devname = args.tty
	s = serial.Serial(devname, 9600)
	if not s:
		print("Failed to open serial port: {}".format(devname))
		return 1

	time.sleep(10)
	s.write(args.command.encode('utf-8'))
	time.sleep(1)

	s.close()

	return 0

	ctx = zmq.Context()
	sock = ctx.socket(zmq.SUB)
	addr = "tcp://" + args.host + ":" + str(DefaultServoPort)
	print("Connecting to: {}".format(addr))
	sock.connect(addr)
	sock.setsockopt(zmq.SUBSCRIBE, b'')

	try:
		while True:
			json = sock.recv_json()
			assert json['name'] == args.name
			op = json['type']
			if op == 'ON':
				s.write(b'i')
				print("TURN ON")
			elif op == 'OFF':
				s.write(b'o')
				print("TURN OFF")
			else:
				print("UNKNOWN MESSAGE: {}".format(op))
	except KeyboardInterrupt:
		s.close()

	return 0

if __name__ == '__main__':
	sys.exit(main())
