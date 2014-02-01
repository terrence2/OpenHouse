CC = gcc
CPP = g++
CFLAGS = -Wall -std=c++0x -I. `pkg-config --cflags libdaemon libzmq` -Os
LIBS = -lbcm2835 `pkg-config --libs libdaemon libzmq`
DEPS = Makefile
OBJ = Network.o MotionDetector.o DHT.o main.o

%.o: %.cpp $(DEPS)
	$(CPP) -c -o $@ $< $(CFLAGS)

nerve: $(OBJ)
	gcc -o $@ $^ $(CFLAGS) $(LIBS)

clean:
	rm -fv nerve *.o

install: nerve
	install -m 755 -o root -g root -d /etc/nerve
	install -m 644 -o root -g root -t /etc/nerve ./config/*
	install -m 755 -o root -g root ./init.d/debian/nerve /etc/init.d/nerve
	install -m 744 -o root -g root ./nerve /usr/bin/nerve
	chmod +s /usr/bin/nerve

