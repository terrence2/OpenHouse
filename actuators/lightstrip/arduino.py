import glob
import serial
import time

class Arduino:
    def __init__(self, devglob, baud):
        self.devglob = devglob
        self.baud = baud
        self.tty = None
        self.tty = self.connect()

    def __del__(self):
        self.close()

    def close(self):
        if self.tty:
            self.tty.close()
            self.tty = None

    def find_device_names(self):
        i = 0
        while True:
            devices = glob.glob(self.devglob)
            if devices:
                return devices
            if i == 0:
                print("No devices like {} found: retrying every 10s".format(self.devglob))
            i += 1
            time.sleep(10)

    def connect(self):
        devnames = self.find_device_names()
        assert devnames
        tty = None
        for name in devnames:
            print("Trying to open arduino at: {}".format(name))
            try:
                tty = serial.Serial(name, self.baud)
            except serial.SerialException as e:
                print("Failed to open arduino: " + str(e))
                continue

        if not tty:
            print("Waiting 10s for arduino to appear in /dev")
            time.sleep(10)
            return self.connect()

        print("Waiting 3s for arduino to reboot...")
        time.sleep(3)
        return tty

    def write(self, data):
        assert self.tty
        try:
            self.tty.write(data)
        except serial.SerialException as e:
            self.tty = self.connect()
            return self.write(data)

