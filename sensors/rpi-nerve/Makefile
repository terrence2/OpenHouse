CC = gcc
CPP = g++
CFLAGS = -Wall -std=c++0x -I. `pkg-config --cflags libzmq`
LIBS = -lbcm2835 `pkg-config --libs libzmq`
DEPS = Makefile
OBJ = Network.o MotionDetector.o DHT.o main.o

%.o: %.cpp $(DEPS)
	$(CPP) -c -O3 -o $@ $< $(CFLAGS)

nerve: $(OBJ)
	gcc -o $@ $^ $(CFLAGS) $(LIBS)

clean:
	rm -fv nerve *.o 
